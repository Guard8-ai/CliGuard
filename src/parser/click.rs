use crate::ir::{Arg, Command, CommandGroup, Flag, Framework, ToolSpec, ValueType};
use crate::parser::Parser;
use crate::security::MAX_SUBCOMMAND_DEPTH;
use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

static RE_COMMAND_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)\s{2,}(.+)$").unwrap());

static RE_CLICK_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:(-\w)\s*,\s*)?(?:(--[\w-]+))(?:\s+(TEXT|INTEGER|INT|FLOAT|BOOL|PATH|FILENAME|UUID|CHOICE|FILE|DIRECTORY)\b)?\s{2,}(.+)$",
    )
    .unwrap()
});

static RE_DEFAULT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[default:\s*([^\]]+)\]").unwrap());

static RE_ENV_VAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[env(?:var)?:\s*([^\]]+)\]").unwrap());

static RE_CLICK_ARG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[?([A-Z][A-Z_]*)\]?").unwrap());

pub struct ClickParser;

impl Parser for ClickParser {
    fn name(&self) -> &'static str {
        "click"
    }

    fn framework(&self) -> Framework {
        Framework::Click
    }

    fn detect(&self, help_output: &str) -> bool {
        // Click shows "Usage:", "Options:", and "--help  Show this message and exit."
        help_output.contains("Show this message and exit.")
            && help_output.contains("Options:")
            && help_output.contains("Usage:")
    }

    fn parse(
        &self,
        binary_name: &str,
        help_output: &str,
        sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        let description = parse_description(help_output);
        let (commands, groups) = parse_click_commands(help_output, binary_name, sub_help, 0);
        let global_flags = parse_click_options(help_output);

        Ok(ToolSpec {
            name: binary_name.to_string(),
            version: None,
            description,
            framework: Framework::Click,
            commands,
            global_flags,
            groups,
        })
    }
}

fn parse_description(help_output: &str) -> String {
    let mut lines = Vec::new();
    let mut past_usage = false;
    for line in help_output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Usage:") {
            past_usage = true;
            continue;
        }
        if !past_usage {
            continue;
        }
        if trimmed.starts_with("Options:") || trimmed.starts_with("Commands:") {
            break;
        }
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    lines.join(" ")
}

fn parse_click_commands(
    help_output: &str,
    binary_name: &str,
    sub_help: &dyn Fn(&str) -> Option<String>,
    depth: usize,
) -> (Vec<Command>, Vec<CommandGroup>) {
    let mut commands = Vec::new();
    let mut group_cmds = Vec::new();
    let mut in_commands = false;

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == "Commands:" {
            in_commands = true;
            continue;
        }

        if in_commands && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }

        if !in_commands || trimmed.is_empty() {
            continue;
        }

        if let Some(cmd) = parse_command_entry(trimmed) {
            if cmd.name.starts_with('-') {
                continue;
            }
            group_cmds.push(cmd.name.clone());

            let mut full_cmd = cmd;
            if depth < MAX_SUBCOMMAND_DEPTH {
                if let Some(sub_output) = sub_help(&full_cmd.name) {
                    let (sub_cmds, _) =
                        parse_click_commands(&sub_output, &format!("{binary_name} {}", full_cmd.name), sub_help, depth + 1);
                    full_cmd.subcommands = sub_cmds;
                    full_cmd.flags = parse_click_options(&sub_output);
                    full_cmd.args = parse_click_args(&sub_output);
                }
            }

            commands.push(full_cmd);
        }
    }

    let groups = if group_cmds.is_empty() {
        Vec::new()
    } else {
        vec![CommandGroup {
            name: "Commands".to_string(),
            commands: group_cmds,
        }]
    };

    (commands, groups)
}

fn parse_command_entry(line: &str) -> Option<Command> {
    RE_COMMAND_ENTRY.captures(line).map(|caps| Command {
        name: caps[1].to_string(),
        description: caps[2].trim().to_string(),
        aliases: Vec::new(),
        subcommands: Vec::new(),
        flags: Vec::new(),
        args: Vec::new(),
        examples: Vec::new(),
    })
}

fn parse_click_options(help_output: &str) -> Vec<Flag> {
    let mut flags = Vec::new();
    let mut in_options = false;

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == "Options:" {
            in_options = true;
            continue;
        }

        if in_options && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }

        if !in_options || trimmed.is_empty() {
            continue;
        }

        if let Some(flag) = parse_click_flag(trimmed) {
            if flag.long.as_deref() == Some("--help") {
                continue;
            }
            flags.push(flag);
        }
    }

    flags
}

fn parse_click_flag(line: &str) -> Option<Flag> {
    // Click format: "  -s, --long TYPE  Description" or "  --long TYPE  Description"
    // TYPE can be: TEXT, INTEGER, FLOAT, BOOL, UUID, PATH, FILENAME, CHOICE
    let caps = RE_CLICK_FLAG.captures(line)?;
    let short = caps.get(1).map(|m| m.as_str().to_string());
    let long = caps.get(2).map(|m| m.as_str().to_string());
    let type_hint = caps.get(3).map(|m| m.as_str().to_string());
    let description = caps[4].trim().to_string();

    let value_type = type_hint.as_deref().map_or(ValueType::Bool, infer_click_type);

    let default = RE_DEFAULT
        .captures(&description)
        .map(|c| c[1].trim().to_string());

    let env_var = RE_ENV_VAR
        .captures(&description)
        .map(|c| c[1].trim().to_string());

    let required = description.contains("[required]");

    Some(Flag {
        short,
        long,
        description,
        value_type,
        required,
        default,
        env_var,
    })
}

fn parse_click_args(help_output: &str) -> Vec<Arg> {
    // Click shows arguments in the Usage line: "Usage: tool [OPTIONS] ARG1 [ARG2]"
    let mut args = Vec::new();
    for line in help_output.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Usage:") {
            continue;
        }
        for cap in RE_CLICK_ARG.captures_iter(trimmed) {
            let name = &cap[1];
            if name == "OPTIONS" || name == "COMMAND" || name == "ARGS" {
                continue;
            }
            let required = !cap[0].starts_with('[');
            args.push(Arg {
                name: name.to_string(),
                description: String::new(),
                value_type: ValueType::String,
                required,
                variadic: name.ends_with('S'),
            });
        }
        break;
    }
    args
}

#[allow(clippy::match_same_arms)]
fn infer_click_type(hint: &str) -> ValueType {
    match hint {
        "TEXT" => ValueType::String,
        "INTEGER" | "INT" => ValueType::Int,
        "FLOAT" => ValueType::Float,
        "BOOL" => ValueType::Bool,
        "PATH" | "FILENAME" | "FILE" | "DIRECTORY" => ValueType::Path,
        _ => ValueType::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CLICK_HELP: &str = "\
Usage: mytool [OPTIONS] COMMAND [ARGS]...

  A tool for processing data files.

Options:
  -v, --verbose          Enable verbose output
  -c, --config PATH      Config file path  [env: MYTOOL_CONFIG]
  -f, --format TEXT      Output format  [default: json]
  --help                 Show this message and exit.

Commands:
  process   Process input files
  validate  Validate configuration
";

    #[test]
    fn detects_click_output() {
        let parser = ClickParser;
        assert!(parser.detect(SAMPLE_CLICK_HELP));
    }

    #[test]
    fn parses_description() {
        let desc = parse_description(SAMPLE_CLICK_HELP);
        assert!(desc.contains("processing data files"));
    }

    #[test]
    fn parses_options() {
        let flags = parse_click_options(SAMPLE_CLICK_HELP);
        assert_eq!(flags.len(), 3); // --help filtered
        assert_eq!(flags[0].long, Some("--verbose".to_string()));
        assert_eq!(flags[1].value_type, ValueType::Path);
        assert_eq!(flags[1].env_var, Some("MYTOOL_CONFIG".to_string()));
        assert_eq!(flags[2].default, Some("json".to_string()));
    }

    #[test]
    fn parses_commands() {
        let no_recurse = |_: &str| -> Option<String> { None };
        let (cmds, groups) = parse_click_commands(SAMPLE_CLICK_HELP, "mytool", &no_recurse, 0);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].name, "process");
        assert_eq!(groups.len(), 1);
    }
}
