use crate::ir::{Arg, Command, CommandGroup, Flag, Framework, ToolSpec, ValueType};
use crate::parser::Parser;
use crate::security::MAX_SUBCOMMAND_DEPTH;
use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

static RE_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[vV]?(\d+\.\d+\.\d+(?:-\w+)?)\b").unwrap());
static RE_COMMAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\S+)\s{2,}(.+)$").unwrap());
static RE_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:(-\w)\s*,\s*)?(?:(--[\w-]+))(?:\s+(\w+))?\s{2,}(.+)$").unwrap()
});
static RE_DEFAULT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(default\s+(.+?)\)").unwrap());

pub struct CobraParser;

impl Parser for CobraParser {
    fn name(&self) -> &'static str {
        "cobra"
    }

    fn framework(&self) -> Framework {
        Framework::Cobra
    }

    fn detect(&self, help_output: &str) -> bool {
        // Standard Cobra: "Available Commands:" + "Flags:"
        // gh-style Cobra: "CORE COMMANDS:" or "COMMANDS:" (uppercase)
        let has_standard = help_output.contains("Available Commands:") && help_output.contains("Flags:");
        let has_uppercase = (help_output.contains("CORE COMMANDS") || help_output.contains("COMMANDS"))
            && (help_output.contains("FLAGS") || help_output.contains("USAGE"));
        has_standard || has_uppercase
    }

    fn parse(
        &self,
        binary_name: &str,
        help_output: &str,
        sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        let description = parse_description(help_output);
        let (commands, groups) = parse_available_commands(help_output, binary_name, sub_help, 0);
        // Try standard headers first, then uppercase variants
        let mut global_flags = parse_flags_section(help_output, "Flags:");
        if global_flags.is_empty() {
            global_flags = parse_flags_section(help_output, "FLAGS");
        }
        let global_flags = global_flags;
        let mut all_flags = parse_flags_section(help_output, "Global Flags:");
        all_flags.extend(global_flags);

        Ok(ToolSpec {
            name: binary_name.to_string(),
            version: parse_version(help_output),
            description,
            framework: Framework::Cobra,
            commands,
            global_flags: all_flags,
            groups,
        })
    }
}

fn parse_version(help_output: &str) -> Option<String> {
    let first_line = help_output.lines().next().unwrap_or("");
    RE_VERSION.captures(first_line).map(|c| c[1].to_string())
}

fn parse_description(help_output: &str) -> String {
    let mut lines = Vec::new();
    for line in help_output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Usage:") || trimmed.starts_with("Available Commands:") || trimmed.starts_with("Flags:") {
            break;
        }
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    lines.join(" ")
}

fn parse_available_commands(
    help_output: &str,
    binary_name: &str,
    sub_help: &dyn Fn(&str) -> Option<String>,
    depth: usize,
) -> (Vec<Command>, Vec<CommandGroup>) {
    let mut commands = Vec::new();
    let mut groups = Vec::new();
    let mut in_section = false;
    let mut current_group = String::from("Commands");
    let mut current_cmds: Vec<String> = Vec::new();

    for line in help_output.lines() {
        let trimmed = line.trim();

        let trimmed_upper = trimmed.to_uppercase();
        if trimmed.ends_with("Commands:") || trimmed_upper.ends_with("COMMANDS") {
            if in_section && !current_cmds.is_empty() {
                groups.push(CommandGroup {
                    name: current_group.clone(),
                    commands: current_cmds.clone(),
                });
                current_cmds.clear();
            }
            in_section = true;
            current_group = trimmed.trim_end_matches(':').trim().to_string();
            continue;
        }

        // Non-command sections end the commands block (Flags:, FLAGS, USAGE, LEARN MORE, etc.)
        if in_section
            && !trimmed.is_empty()
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && !trimmed_upper.ends_with("COMMANDS")
            && (trimmed.ends_with(':') || trimmed_upper.starts_with("FLAG") || trimmed_upper.starts_with("USAGE") || trimmed_upper.starts_with("LEARN"))
        {
            if !current_cmds.is_empty() {
                groups.push(CommandGroup {
                    name: current_group.clone(),
                    commands: current_cmds.clone(),
                });
                current_cmds.clear();
            }
            in_section = false;
            continue;
        }

        if !in_section || trimmed.is_empty() {
            continue;
        }

        if let Some(cmd) = parse_command_entry(trimmed) {
            if cmd.name == "help" || cmd.name == "completion" || cmd.name.starts_with('-') {
                continue;
            }
            current_cmds.push(cmd.name.clone());

            let mut full_cmd = cmd;
            if depth < MAX_SUBCOMMAND_DEPTH {
                if let Some(sub_output) = sub_help(&full_cmd.name) {
                    let (sub_cmds, _) =
                        parse_available_commands(&sub_output, &format!("{binary_name} {}", full_cmd.name), sub_help, depth + 1);
                    full_cmd.subcommands = sub_cmds;
                    full_cmd.flags = parse_flags_section(&sub_output, "Flags:");
                    full_cmd.args = parse_args_from_usage(&sub_output);
                }
            }

            commands.push(full_cmd);
        }
    }

    if !current_cmds.is_empty() {
        groups.push(CommandGroup {
            name: current_group,
            commands: current_cmds,
        });
    }

    (commands, groups)
}

fn parse_command_entry(line: &str) -> Option<Command> {
    RE_COMMAND.captures(line).map(|caps| Command {
        name: caps[1].trim_end_matches(':').to_string(),
        description: caps[2].trim().to_string(),
        aliases: Vec::new(),
        subcommands: Vec::new(),
        flags: Vec::new(),
        args: Vec::new(),
        examples: Vec::new(),
    })
}

fn parse_flags_section(help_output: &str, section_header: &str) -> Vec<Flag> {
    let mut flags = Vec::new();
    let mut in_flags = false;

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == section_header {
            in_flags = true;
            continue;
        }

        if in_flags && !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }

        if !in_flags || trimmed.is_empty() {
            continue;
        }

        if let Some(flag) = parse_cobra_flag(trimmed) {
            if flag.long.as_deref() == Some("--help") {
                continue;
            }
            flags.push(flag);
        }
    }

    flags
}

fn parse_cobra_flag(line: &str) -> Option<Flag> {
    let caps = RE_FLAG.captures(line)?;
    let short = caps.get(1).map(|m| m.as_str().to_string());
    let long = caps.get(2).map(|m| m.as_str().to_string());
    let type_hint = caps.get(3).map(|m| m.as_str().to_string());
    let description = caps[4].trim().to_string();

    let value_type = type_hint
        .as_deref()
        .map_or(ValueType::Bool, infer_cobra_type);

    let default = RE_DEFAULT
        .captures(&description)
        .map(|c| c[1].trim().to_string());

    Some(Flag {
        short,
        long,
        description,
        value_type,
        required: false,
        default,
        env_var: None,
    })
}

fn parse_args_from_usage(_help_output: &str) -> Vec<Arg> {
    // Cobra doesn't have a formal args section like clap; args are in the usage line
    // but without a standard format for extraction.
    Vec::new()
}

#[allow(clippy::match_same_arms)]
fn infer_cobra_type(hint: &str) -> ValueType {
    match hint {
        "string" | "stringArray" => ValueType::String,
        "int" | "int32" | "int64" | "uint" => ValueType::Int,
        "float32" | "float64" => ValueType::Float,
        "bool" => ValueType::Bool,
        _ => ValueType::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_COBRA_HELP: &str = "\
Tool for managing things

Usage:
  tool [command]

Available Commands:
  create      Create a new resource
  delete      Delete a resource
  list        List resources
  help        Help about any command

Additional Commands:
  config      Manage configuration

Flags:
  -h, --help           help for tool
  -o, --output string  Output format (default \"json\")
  -v, --verbose        Enable verbose output

Global Flags:
      --config string  Config file path

Use \"tool [command] --help\" for more information about a command.
";

    #[test]
    fn detects_cobra_output() {
        let parser = CobraParser;
        assert!(parser.detect(SAMPLE_COBRA_HELP));
    }

    #[test]
    fn does_not_detect_clap() {
        let parser = CobraParser;
        let clap_help = "Usage: tool [OPTIONS]\n\nOptions:\n  -h, --help  Print help";
        assert!(!parser.detect(clap_help));
    }

    #[test]
    fn parses_description() {
        let desc = parse_description(SAMPLE_COBRA_HELP);
        assert!(desc.contains("managing things"));
    }

    #[test]
    fn parses_flags() {
        let flags = parse_flags_section(SAMPLE_COBRA_HELP, "Flags:");
        assert_eq!(flags.len(), 2);
        assert_eq!(flags[0].long, Some("--output".to_string()));
        assert_eq!(flags[0].value_type, ValueType::String);
    }

    #[test]
    fn parses_global_flags() {
        let flags = parse_flags_section(SAMPLE_COBRA_HELP, "Global Flags:");
        assert_eq!(flags.len(), 1);
        assert_eq!(flags[0].long, Some("--config".to_string()));
    }

    #[test]
    fn parses_commands_with_groups() {
        let no_recurse = |_: &str| -> Option<String> { None };
        let (cmds, groups) = parse_available_commands(SAMPLE_COBRA_HELP, "tool", &no_recurse, 0);
        assert_eq!(cmds.len(), 4);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Available Commands");
        assert_eq!(groups[1].name, "Additional Commands");
    }
}
