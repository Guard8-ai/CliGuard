use crate::ir::{Arg, Command, CommandGroup, Flag, Framework, ToolSpec, ValueType};
use crate::parser::Parser;
use crate::security::MAX_SUBCOMMAND_DEPTH;
use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

static RE_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d+\.\d+\.\d+(?:[.-]\w+)?)\b").unwrap());

static RE_SECTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(optional arguments|options):").unwrap());

static RE_ARGPARSE_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(-\w)\s*(?:,\s*)?)?(?:(--[\w-]+))?\s*(?:(\w+(?:\s+\w+)*?))?\s{2,}(.+)$")
        .unwrap()
});

static RE_DEFAULT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(default:\s*(.+?)\)").unwrap());

static RE_CHOICES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]+)\}").unwrap());

static RE_POSITIONAL_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\w+)\s{2,}(.+)$").unwrap());

static RE_SUBPARSER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{([^}]+)\}").unwrap());

static RE_SUBCOMMAND_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\w[\w-]*)\s{2,}(.+)$").unwrap());

pub struct ArgparseParser;

impl Parser for ArgparseParser {
    fn name(&self) -> &'static str {
        "argparse"
    }

    fn framework(&self) -> Framework {
        Framework::Argparse
    }

    fn detect(&self, help_output: &str) -> bool {
        // Argparse shows "usage:" (lowercase), "positional arguments:", and "optional arguments:" or "options:"
        let has_usage = help_output.contains("usage:");
        let has_positional = help_output.contains("positional arguments:");
        let has_optional =
            help_output.contains("optional arguments:") || help_output.contains("options:");
        // Must have usage and at least one of positional/optional
        has_usage && (has_positional || has_optional)
    }

    fn parse(
        &self,
        binary_name: &str,
        help_output: &str,
        sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        let description = parse_description(help_output);
        let global_flags = parse_optional_args(help_output);
        let positional = parse_positional_args(help_output);
        let (commands, groups) = parse_subparsers(help_output, binary_name, sub_help, 0);

        // If there are positional args that aren't subcommands, attach them
        // to the tool spec via a synthetic root command or as top-level args
        let mut all_commands = commands;
        if !positional.is_empty() && all_commands.is_empty() {
            // No subcommands, positional args belong to the root command
            all_commands.push(Command {
                name: binary_name.to_string(),
                description: description.clone(),
                aliases: Vec::new(),
                subcommands: Vec::new(),
                flags: Vec::new(),
                args: positional,
                examples: Vec::new(),
            });
        }

        Ok(ToolSpec {
            name: binary_name.to_string(),
            version: parse_version(help_output),
            description,
            framework: Framework::Argparse,
            commands: all_commands,
            global_flags,
            groups,
        })
    }
}

fn parse_version(help_output: &str) -> Option<String> {
    // Some argparse tools have --version in options with a version number
    // Check the first few lines
    for line in help_output.lines().take(5) {
        if let Some(m) = RE_VERSION.find(line) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

fn parse_description(help_output: &str) -> String {
    let mut lines = Vec::new();
    let mut past_usage = false;
    for line in help_output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("usage:") {
            past_usage = true;
            continue;
        }
        if !past_usage {
            continue;
        }
        // Skip continuation of usage line (starts with whitespace)
        if line.starts_with(' ') && lines.is_empty() {
            continue;
        }
        if trimmed.starts_with("positional arguments:")
            || trimmed.starts_with("optional arguments:")
            || trimmed.starts_with("options:")
            || trimmed.starts_with("subcommands:")
        {
            break;
        }
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    lines.join(" ")
}

fn parse_optional_args(help_output: &str) -> Vec<Flag> {
    let mut flags = Vec::new();
    let mut in_section = false;
    let mut current_flag_text = String::new();

    for line in help_output.lines() {
        let trimmed = line.trim();

        if RE_SECTION.is_match(trimmed) {
            in_section = true;
            continue;
        }

        if in_section && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            // Process last accumulated flag
            if !current_flag_text.is_empty() {
                if let Some(flag) = parse_argparse_flag(&current_flag_text) {
                    flags.push(flag);
                }
            }
            break;
        }

        if !in_section {
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Detect if this is a new flag line (starts with -)
        if trimmed.starts_with('-') {
            if !current_flag_text.is_empty() {
                if let Some(flag) = parse_argparse_flag(&current_flag_text) {
                    flags.push(flag);
                }
            }
            current_flag_text = trimmed.to_string();
        } else {
            // Continuation of previous flag description
            current_flag_text.push(' ');
            current_flag_text.push_str(trimmed);
        }
    }

    // Don't forget the last one
    if !current_flag_text.is_empty() {
        if let Some(flag) = parse_argparse_flag(&current_flag_text) {
            flags.push(flag);
        }
    }

    // Filter help/version
    flags
        .into_iter()
        .filter(|f| f.long.as_deref() != Some("--help") && f.long.as_deref() != Some("--version"))
        .collect()
}

fn parse_argparse_flag(line: &str) -> Option<Flag> {
    // Patterns:
    //   -s, --long METAVAR  Description
    //   -s METAVAR          Description
    //   --long METAVAR      Description
    let caps = RE_ARGPARSE_FLAG.captures(line)?;
    let short = caps.get(1).map(|m| m.as_str().to_string());
    let long = caps.get(2).map(|m| m.as_str().to_string());
    let metavar = caps.get(3).map(|m| m.as_str().to_string());
    let description = caps[4].trim().to_string();

    if short.is_none() && long.is_none() {
        return None;
    }

    let value_type = metavar
        .as_deref()
        .map_or(ValueType::Bool, infer_argparse_type);

    let default = RE_DEFAULT
        .captures(&description)
        .map(|c| c[1].trim().to_string());

    // Check for choices: {a,b,c}
    let choices = RE_CHOICES.captures(&description).map(|c| {
        c[1].split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    });

    let final_type = if let Some(ref choices) = choices {
        ValueType::Enum(choices.clone())
    } else {
        value_type
    };

    Some(Flag {
        short,
        long,
        description,
        value_type: final_type,
        required: false,
        default,
        env_var: None,
    })
}

fn parse_positional_args(help_output: &str) -> Vec<Arg> {
    let mut args = Vec::new();
    let mut in_positional = false;

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == "positional arguments:" {
            in_positional = true;
            continue;
        }

        if in_positional && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t')
        {
            break;
        }

        if !in_positional || trimmed.is_empty() {
            continue;
        }

        // Skip {cmd1,cmd2} subparser entries
        if trimmed.starts_with('{') {
            continue;
        }

        if let Some(arg) = parse_positional_entry(trimmed) {
            args.push(arg);
        }
    }

    args
}

fn parse_positional_entry(line: &str) -> Option<Arg> {
    let caps = RE_POSITIONAL_ENTRY.captures(line)?;
    Some(Arg {
        name: caps[1].to_string(),
        description: caps[2].trim().to_string(),
        value_type: ValueType::String,
        required: true,
        variadic: false,
    })
}

fn parse_subparsers(
    help_output: &str,
    binary_name: &str,
    sub_help: &dyn Fn(&str) -> Option<String>,
    depth: usize,
) -> (Vec<Command>, Vec<CommandGroup>) {
    // Argparse subparsers appear as "{cmd1,cmd2,cmd3}" in positional args
    // or as a labeled section
    let mut commands = Vec::new();
    let mut group_cmds = Vec::new();
    let mut in_subcommands = false;

    // Look for subcommands section or {cmd1,cmd2} pattern

    for line in help_output.lines() {
        let trimmed = line.trim();

        // Detect subcommand sections
        if trimmed == "subcommands:" || trimmed.ends_with("sub-commands:") {
            in_subcommands = true;
            continue;
        }

        if in_subcommands
            && !trimmed.is_empty()
            && !line.starts_with(' ')
            && !line.starts_with('\t')
        {
            break;
        }

        if in_subcommands && !trimmed.is_empty() && !trimmed.starts_with('{') {
            if let Some(cmd) = parse_subcommand_entry(trimmed) {
                group_cmds.push(cmd.name.clone());
                let mut full_cmd = cmd;
                if depth < MAX_SUBCOMMAND_DEPTH {
                    if let Some(sub_output) = sub_help(&full_cmd.name) {
                        full_cmd.flags = parse_optional_args(&sub_output);
                        full_cmd.args = parse_positional_args(&sub_output);
                    }
                }
                commands.push(full_cmd);
            }
            continue;
        }

        // Also check for inline {cmd1,cmd2,...} in positional args and usage line
        if let Some(caps) = RE_SUBPARSER.captures(trimmed) {
            let cmds_str = &caps[1];
            for cmd_name in cmds_str.split(',') {
                let cmd_name = cmd_name.trim();
                if cmd_name.is_empty() || group_cmds.contains(&cmd_name.to_string()) {
                    continue;
                }
                group_cmds.push(cmd_name.to_string());
                let mut cmd = Command {
                    name: cmd_name.to_string(),
                    description: String::new(),
                    aliases: Vec::new(),
                    subcommands: Vec::new(),
                    flags: Vec::new(),
                    args: Vec::new(),
                    examples: Vec::new(),
                };
                if depth < MAX_SUBCOMMAND_DEPTH {
                    if let Some(sub_output) = sub_help(cmd_name) {
                        cmd.description = parse_description(&sub_output);
                        cmd.flags = parse_optional_args(&sub_output);
                        cmd.args = parse_positional_args(&sub_output);
                        let sub_name = format!("{binary_name} {cmd_name}");
                        let (sub_cmds, _) =
                            parse_subparsers(&sub_output, &sub_name, sub_help, depth + 1);
                        cmd.subcommands = sub_cmds;
                    }
                }
                commands.push(cmd);
            }
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

fn parse_subcommand_entry(line: &str) -> Option<Command> {
    RE_SUBCOMMAND_ENTRY.captures(line).map(|caps| Command {
        name: caps[1].to_string(),
        description: caps[2].trim().to_string(),
        aliases: Vec::new(),
        subcommands: Vec::new(),
        flags: Vec::new(),
        args: Vec::new(),
        examples: Vec::new(),
    })
}

fn infer_argparse_type(metavar: &str) -> ValueType {
    match metavar.to_uppercase().as_str() {
        "FILE" | "PATH" | "DIR" | "DIRECTORY" | "FILENAME" => ValueType::Path,
        "N" | "NUM" | "COUNT" | "INT" | "INTEGER" => ValueType::Int,
        "FLOAT" => ValueType::Float,
        _ => ValueType::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ARGPARSE_HELP: &str = "\
usage: mytool [-h] [--verbose] [--output FILE] {process,validate} ...

A data processing tool

positional arguments:
  {process,validate}  Command to run
  input               Input file path

optional arguments:
  -h, --help          show this help message and exit
  --verbose           Enable verbose mode
  -o, --output FILE   Output file path (default: stdout)
";

    #[test]
    fn detects_argparse_output() {
        let parser = ArgparseParser;
        assert!(parser.detect(SAMPLE_ARGPARSE_HELP));
    }

    #[test]
    fn does_not_detect_clap() {
        let parser = ArgparseParser;
        let clap_help = "Usage: tool [OPTIONS]\n\nOptions:\n  -h, --help  Print help";
        assert!(!parser.detect(clap_help));
    }

    #[test]
    fn parses_description() {
        let desc = parse_description(SAMPLE_ARGPARSE_HELP);
        assert!(desc.contains("data processing tool"));
    }

    #[test]
    fn parses_optional_args() {
        let flags = parse_optional_args(SAMPLE_ARGPARSE_HELP);
        assert_eq!(flags.len(), 2); // --help filtered
        assert_eq!(flags[0].long, Some("--verbose".to_string()));
        assert_eq!(flags[1].value_type, ValueType::Path);
    }

    #[test]
    fn parses_positional_args() {
        let args = parse_positional_args(SAMPLE_ARGPARSE_HELP);
        assert_eq!(args.len(), 1); // {process,validate} skipped, input parsed
        assert_eq!(args[0].name, "input");
    }

    #[test]
    fn parses_subparsers() {
        let no_recurse = |_: &str| -> Option<String> { None };
        let (cmds, _) = parse_subparsers(SAMPLE_ARGPARSE_HELP, "mytool", &no_recurse, 0);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].name, "process");
        assert_eq!(cmds[1].name, "validate");
    }
}
