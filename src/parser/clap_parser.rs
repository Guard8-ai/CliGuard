use crate::ir::{Arg, Command, CommandGroup, Flag, Framework, ToolSpec, ValueType};
use crate::parser::Parser;
use crate::security::MAX_SUBCOMMAND_DEPTH;
use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

static RE_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d+\.\d+\.\d+(?:-\w+)?)\b").unwrap());
static RE_VERSION_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d+\.\d+\.\d+").unwrap());
static RE_COMMAND_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)\s{2,}(.+)$").unwrap());
static RE_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:(-\w)\s*,\s*)?(?:(--[\w-]+))?(?:\s+<(\w+)>)?\s{2,}(.+)$").unwrap()
});
static RE_DEFAULT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[default:\s*([^\]]+)\]").unwrap());
static RE_ENV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[env:\s*([^\]]+)\]").unwrap());
static RE_ARG_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[<\[](\w+)[>\]]\s{2,}(.+)$").unwrap());

pub struct ClapParser;

impl Parser for ClapParser {
    fn name(&self) -> &'static str {
        "clap"
    }

    fn framework(&self) -> Framework {
        Framework::Clap
    }

    fn detect(&self, help_output: &str) -> bool {
        let has_usage = help_output.contains("Usage:");
        let has_commands =
            help_output.contains("Commands:") || help_output.contains("Subcommands:");
        let has_options = help_output.contains("Options:");
        let has_help_flag = help_output.contains("-h, --help");

        has_usage
            && has_options
            && has_help_flag
            && (has_commands || !help_output.contains("Available Commands:"))
    }

    fn parse(
        &self,
        binary_name: &str,
        help_output: &str,
        sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        let description = parse_description(help_output);
        let (commands, groups) = parse_commands_section(help_output, binary_name, sub_help, 0);
        let global_flags = parse_options_section(help_output);

        Ok(ToolSpec {
            name: binary_name.to_string(),
            version: parse_version(help_output),
            description,
            framework: Framework::Clap,
            commands,
            global_flags,
            groups,
        })
    }
}

fn parse_version(help_output: &str) -> Option<String> {
    let first_line = help_output.lines().next().unwrap_or("");
    RE_VERSION.find(first_line).map(|m| m.as_str().to_string())
}

fn parse_description(help_output: &str) -> String {
    let mut lines = Vec::new();
    let mut found_first = false;
    for line in help_output.lines() {
        if !found_first {
            found_first = true;
            if RE_VERSION_LINE.is_match(line) {
                continue;
            }
        }
        if line.starts_with("Usage:") || line.starts_with("usage:") {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    lines.join(" ")
}

fn parse_commands_section(
    help_output: &str,
    binary_name: &str,
    sub_help: &dyn Fn(&str) -> Option<String>,
    depth: usize,
) -> (Vec<Command>, Vec<CommandGroup>) {
    let mut commands = Vec::new();
    let mut groups = Vec::new();
    let mut in_commands = false;
    let mut current_group_name = String::from("Commands");
    let mut current_group_cmds: Vec<String> = Vec::new();

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed.ends_with(':') && !trimmed.starts_with('-') && !trimmed.starts_with(' ') {
            if in_commands && !current_group_cmds.is_empty() {
                groups.push(CommandGroup {
                    name: current_group_name.clone(),
                    commands: current_group_cmds.clone(),
                });
                current_group_cmds.clear();
            }

            if trimmed == "Commands:" || trimmed == "Subcommands:" {
                in_commands = true;
                current_group_name = trimmed.trim_end_matches(':').to_string();
            } else if in_commands
                && !trimmed.eq_ignore_ascii_case("options:")
                && !trimmed.eq_ignore_ascii_case("usage:")
                && !trimmed.eq_ignore_ascii_case("arguments:")
            {
                current_group_name = trimmed.trim_end_matches(':').to_string();
            } else if in_commands {
                in_commands = false;
            }
            continue;
        }

        if !in_commands || trimmed.is_empty() {
            continue;
        }

        if let Some(cmd) = parse_command_line(trimmed) {
            if cmd.name == "help" || cmd.name.starts_with('-') {
                continue;
            }
            current_group_cmds.push(cmd.name.clone());

            let sub_key = format!("{binary_name} {}", cmd.name);
            let mut full_cmd = cmd;
            if depth < MAX_SUBCOMMAND_DEPTH {
                if let Some(sub_output) = sub_help(&full_cmd.name) {
                    let (sub_cmds, _) =
                        parse_commands_section(&sub_output, &sub_key, sub_help, depth + 1);
                    full_cmd.subcommands = sub_cmds;
                    full_cmd.flags = parse_options_section(&sub_output);
                    full_cmd.args = parse_args_section(&sub_output);
                }
            }

            commands.push(full_cmd);
        }
    }

    if !current_group_cmds.is_empty() {
        groups.push(CommandGroup {
            name: current_group_name,
            commands: current_group_cmds,
        });
    }

    (commands, groups)
}

fn parse_command_line(line: &str) -> Option<Command> {
    RE_COMMAND_LINE.captures(line).map(|caps| Command {
        name: caps[1].to_string(),
        description: caps[2].trim().to_string(),
        aliases: Vec::new(),
        subcommands: Vec::new(),
        flags: Vec::new(),
        args: Vec::new(),
        examples: Vec::new(),
    })
}

fn parse_options_section(help_output: &str) -> Vec<Flag> {
    let mut flags = Vec::new();
    let mut in_options = false;

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == "Options:" || trimmed == "options:" {
            in_options = true;
            continue;
        }

        if in_options && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }

        if !in_options || trimmed.is_empty() {
            continue;
        }

        if let Some(flag) = parse_flag_line(trimmed) {
            if flag.long.as_deref() == Some("--help") || flag.long.as_deref() == Some("--version") {
                continue;
            }
            flags.push(flag);
        }
    }

    flags
}

fn parse_flag_line(line: &str) -> Option<Flag> {
    let caps = RE_FLAG.captures(line)?;

    let short = caps.get(1).map(|m| m.as_str().to_string());
    let long = caps.get(2).map(|m| m.as_str().to_string());
    let value_hint = caps.get(3).map(|m| m.as_str().to_string());
    let description = caps[4].trim().to_string();

    if short.is_none() && long.is_none() {
        return None;
    }

    let (value_type, required) = if let Some(ref hint) = value_hint {
        (infer_value_type(hint), true)
    } else {
        (ValueType::Bool, false)
    };

    let default = RE_DEFAULT
        .captures(&description)
        .map(|c| c[1].trim().to_string());

    let env_var = RE_ENV
        .captures(&description)
        .map(|c| c[1].trim().to_string());

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

fn parse_args_section(help_output: &str) -> Vec<Arg> {
    let mut args = Vec::new();
    let mut in_args = false;

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == "Arguments:" || trimmed == "Args:" {
            in_args = true;
            continue;
        }

        if in_args && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }

        if !in_args || trimmed.is_empty() {
            continue;
        }

        if let Some(arg) = parse_arg_line(trimmed) {
            args.push(arg);
        }
    }

    args
}

fn parse_arg_line(line: &str) -> Option<Arg> {
    let caps = RE_ARG_LINE.captures(line)?;
    let name = caps[1].to_string();
    let description = caps[2].trim().to_string();
    let required = line.contains('<');
    let variadic = line.contains("...");

    Some(Arg {
        name,
        description,
        value_type: ValueType::String,
        required,
        variadic,
    })
}

fn infer_value_type(hint: &str) -> ValueType {
    match hint.to_uppercase().as_str() {
        "PATH" | "FILE" | "DIR" | "DIRECTORY" => ValueType::Path,
        "NUM" | "NUMBER" | "COUNT" | "N" | "INT" | "INTEGER" => ValueType::Int,
        "FLOAT" | "DECIMAL" => ValueType::Float,
        "BOOL" | "BOOLEAN" => ValueType::Bool,
        _ => ValueType::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CLAP_HELP: &str = "\
cliguard 0.1.0
Auto-generate agentic AI guides from CLI help output

Usage: cliguard [OPTIONS] <BINARY>

Arguments:
  <BINARY>  The CLI binary to generate a guide for

Options:
  -f, --framework <FRAMEWORK>  Force a specific framework parser
  -o, --output <PATH>          Output file path
      --format <FORMAT>        Output format [default: md]
  -h, --help                   Print help
  -V, --version                Print version

Commands:
  parse     Parse a binary and output IR
  generate  Generate a guide from IR
  help      Print this message or the help of the given subcommand(s)
";

    #[test]
    fn detects_clap_output() {
        let parser = ClapParser;
        assert!(parser.detect(SAMPLE_CLAP_HELP));
    }

    #[test]
    fn does_not_detect_cobra() {
        let parser = ClapParser;
        let cobra_help = "Usage:\n  tool [command]\n\nAvailable Commands:\n  foo  Do foo\n\nFlags:\n  -h, --help   help for tool";
        assert!(!parser.detect(cobra_help));
    }

    #[test]
    fn parses_description() {
        let desc = parse_description(SAMPLE_CLAP_HELP);
        assert!(desc.contains("Auto-generate agentic AI guides"));
    }

    #[test]
    fn parses_version() {
        let ver = parse_version(SAMPLE_CLAP_HELP);
        assert_eq!(ver, Some("0.1.0".to_string()));
    }

    #[test]
    fn parses_options() {
        let flags = parse_options_section(SAMPLE_CLAP_HELP);
        assert_eq!(flags.len(), 3);
        assert_eq!(flags[0].long, Some("--framework".to_string()));
        assert_eq!(flags[1].long, Some("--output".to_string()));
        assert_eq!(flags[1].value_type, ValueType::Path);
        assert_eq!(flags[2].default, Some("md".to_string()));
    }

    #[test]
    fn parses_args() {
        let args = parse_args_section(SAMPLE_CLAP_HELP);
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "BINARY");
        assert!(args[0].required);
    }

    #[test]
    fn parses_commands() {
        let no_recurse = |_: &str| -> Option<String> { None };
        let (cmds, groups) = parse_commands_section(SAMPLE_CLAP_HELP, "cliguard", &no_recurse, 0);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].name, "parse");
        assert_eq!(groups.len(), 1);
    }
}
