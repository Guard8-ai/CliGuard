use crate::ir::{Command, CommandGroup, Flag, Framework, ToolSpec, ValueType};
use crate::parser::Parser;
use crate::security::MAX_SUBCOMMAND_DEPTH;
use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

static RE_GCLOUD_FLAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{5}(--[\w-]+)(?:=([\w\[\],._]+))?\s*$").unwrap());
static RE_GCLOUD_SHORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{5}(-\w),\s+(--[\w-]+)").unwrap());
static RE_GCLOUD_ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{5}([\w][\w-]*)\s*$").unwrap());

pub struct GcloudParser;

impl Parser for GcloudParser {
    fn name(&self) -> &'static str {
        "gcloud"
    }

    fn framework(&self) -> Framework {
        Framework::Unknown
    }

    fn detect(&self, help_output: &str) -> bool {
        help_output.contains("NAME")
            && help_output.contains("SYNOPSIS")
            && (help_output.contains("GLOBAL FLAGS") || help_output.contains("GCLOUD WIDE FLAGS"))
    }

    fn parse(
        &self,
        binary_name: &str,
        help_output: &str,
        sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        let description = parse_gcloud_description(help_output);
        let global_flags = parse_gcloud_flags(help_output, "GLOBAL FLAGS");
        let (groups_cmds, groups) = parse_gcloud_commands(help_output, binary_name, sub_help);

        Ok(ToolSpec {
            name: binary_name.to_string(),
            version: None,
            description,
            framework: Framework::Unknown,
            commands: groups_cmds,
            global_flags,
            groups,
        })
    }
}

fn parse_gcloud_description(help_output: &str) -> String {
    let mut in_desc = false;
    let mut lines = Vec::new();

    for line in help_output.lines() {
        let trimmed = line.trim();
        if trimmed == "DESCRIPTION" {
            in_desc = true;
            continue;
        }
        if in_desc && !trimmed.is_empty() && !line.starts_with(' ') {
            break;
        }
        if in_desc && !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    lines.join(" ")
}

fn parse_gcloud_flags(help_output: &str, section: &str) -> Vec<Flag> {
    let mut flags = Vec::new();
    let mut in_section = false;
    let mut current_flag: Option<(String, Option<String>, String)> = None;
    let mut current_desc = String::new();

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == section {
            in_section = true;
            continue;
        }

        if in_section && !trimmed.is_empty() && !line.starts_with(' ') {
            // Save last flag
            if let Some((long, value, _short)) = current_flag.take() {
                flags.push(build_flag(None, &long, value.as_deref(), &current_desc));
                current_desc.clear();
            }
            break;
        }

        if !in_section {
            continue;
        }

        // Check for short,long pattern: "  -q, --quiet"
        if let Some(caps) = RE_GCLOUD_SHORT.captures(line) {
            if let Some((long, value, _short)) = current_flag.take() {
                flags.push(build_flag(None, &long, value.as_deref(), &current_desc));
                current_desc.clear();
            }
            current_flag = Some((caps[2].to_string(), None, caps[1].to_string()));
            continue;
        }

        // Check for flag line: "     --flag=VALUE"
        if let Some(caps) = RE_GCLOUD_FLAG.captures(line) {
            if let Some((long, value, short)) = current_flag.take() {
                flags.push(build_flag(
                    if short.is_empty() { None } else { Some(&short) },
                    &long,
                    value.as_deref(),
                    &current_desc,
                ));
                current_desc.clear();
            }
            current_flag = Some((
                caps[1].to_string(),
                caps.get(2).map(|m| m.as_str().to_string()),
                String::new(),
            ));
            continue;
        }

        // Description lines (8+ spaces indent)
        if line.starts_with("        ") && current_flag.is_some() {
            if !current_desc.is_empty() {
                current_desc.push(' ');
            }
            current_desc.push_str(trimmed);
        }
    }

    if let Some((long, value, short)) = current_flag {
        flags.push(build_flag(
            if short.is_empty() { None } else { Some(&short) },
            &long,
            value.as_deref(),
            &current_desc,
        ));
    }

    // Filter help/version
    flags
        .into_iter()
        .filter(|f| f.long.as_deref() != Some("--help") && f.long.as_deref() != Some("--version"))
        .collect()
}

fn build_flag(short: Option<&str>, long: &str, value_hint: Option<&str>, desc: &str) -> Flag {
    let value_type = if value_hint.is_some() {
        ValueType::String
    } else {
        ValueType::Bool
    };

    Flag {
        short: short.map(String::from),
        long: Some(long.to_string()),
        description: desc.to_string(),
        value_type,
        required: false,
        default: None,
        env_var: None,
    }
}

fn parse_gcloud_commands(
    help_output: &str,
    binary_name: &str,
    sub_help: &dyn Fn(&str) -> Option<String>,
) -> (Vec<Command>, Vec<CommandGroup>) {
    let mut commands = Vec::new();
    let mut groups = Vec::new();

    for section in ["GROUPS", "COMMANDS"] {
        let (cmds, group_names) = parse_gcloud_section(help_output, section, binary_name, sub_help);
        if !group_names.is_empty() {
            groups.push(CommandGroup {
                name: section.to_string(),
                commands: group_names,
            });
        }
        commands.extend(cmds);
    }

    (commands, groups)
}

fn parse_gcloud_section(
    help_output: &str,
    section: &str,
    binary_name: &str,
    sub_help: &dyn Fn(&str) -> Option<String>,
) -> (Vec<Command>, Vec<String>) {
    let mut commands = Vec::new();
    let mut names = Vec::new();
    let mut in_section = false;
    let mut current_name: Option<String> = None;
    let mut current_desc = String::new();

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed == section {
            in_section = true;
            continue;
        }

        if in_section && !trimmed.is_empty() && !line.starts_with(' ') {
            // Save last command
            if let Some(name) = current_name.take() {
                names.push(name.clone());
                commands.push(make_cmd(&name, &current_desc, binary_name, sub_help));
                current_desc.clear();
            }
            break;
        }

        if !in_section {
            continue;
        }

        // Skip the "GROUP/COMMAND is one of the following:" line
        if trimmed.contains("is one of the following") {
            continue;
        }

        // Command name line (5-space indent, single word)
        if let Some(caps) = RE_GCLOUD_ENTRY.captures(line) {
            if let Some(name) = current_name.take() {
                names.push(name.clone());
                commands.push(make_cmd(&name, &current_desc, binary_name, sub_help));
                current_desc.clear();
            }
            current_name = Some(caps[1].to_string());
            continue;
        }

        // Description line (8+ spaces)
        if line.starts_with("        ") && current_name.is_some() {
            if !current_desc.is_empty() {
                current_desc.push(' ');
            }
            current_desc.push_str(trimmed);
        }
    }

    if let Some(name) = current_name {
        names.push(name.clone());
        commands.push(make_cmd(&name, &current_desc, binary_name, sub_help));
    }

    (commands, names)
}

fn make_cmd(
    name: &str,
    desc: &str,
    binary_name: &str,
    sub_help: &dyn Fn(&str) -> Option<String>,
) -> Command {
    let cmd = Command {
        name: name.to_string(),
        description: desc.to_string(),
        aliases: Vec::new(),
        subcommands: Vec::new(),
        flags: Vec::new(),
        args: Vec::new(),
        examples: Vec::new(),
    };

    // Don't recurse for gcloud — too many subgroups and it's slow
    let _ = (binary_name, sub_help, MAX_SUBCOMMAND_DEPTH);

    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_GCLOUD: &str = "\
NAME
    gcloud - manage Google Cloud resources

SYNOPSIS
    gcloud GROUP | COMMAND [--account=ACCOUNT] [--project=PROJECT_ID]

DESCRIPTION
    The gcloud CLI manages authentication and interactions with Google Cloud.

GLOBAL FLAGS
     --account=ACCOUNT
        Google Cloud user account to use.

     --project=PROJECT_ID
        The Google Cloud project ID.

     -q, --quiet
        Disable all interactive prompts.

GROUPS
    GROUP is one of the following:

     compute
        Create and manage Compute Engine resources.

     storage
        Create and manage Cloud Storage resources.

COMMANDS
    COMMAND is one of the following:

     init
        Initialize or reinitialize gcloud.

     info
        Display information about the current gcloud environment.

     help
        Search gcloud help text.
";

    #[test]
    fn detects_gcloud() {
        let parser = GcloudParser;
        assert!(parser.detect(SAMPLE_GCLOUD));
    }

    #[test]
    fn does_not_detect_clap() {
        let parser = GcloudParser;
        assert!(!parser.detect("Usage: tool [OPTIONS]\n\nOptions:\n  -h, --help"));
    }

    #[test]
    fn parses_description() {
        let desc = parse_gcloud_description(SAMPLE_GCLOUD);
        assert!(desc.contains("manages authentication"));
    }

    #[test]
    fn parses_global_flags() {
        let flags = parse_gcloud_flags(SAMPLE_GCLOUD, "GLOBAL FLAGS");
        assert!(flags.len() >= 2);
        let account = flags
            .iter()
            .find(|f| f.long.as_deref() == Some("--account"));
        assert!(account.is_some());
        let quiet = flags.iter().find(|f| f.long.as_deref() == Some("--quiet"));
        assert!(quiet.is_some());
    }

    #[test]
    fn parses_groups_and_commands() {
        let no_recurse = |_: &str| -> Option<String> { None };
        let (cmds, groups) = parse_gcloud_commands(SAMPLE_GCLOUD, "gcloud", &no_recurse);
        assert!(cmds.len() >= 4); // compute, storage, init, info (help filtered by default in guide gen)
        assert_eq!(groups.len(), 2); // GROUPS, COMMANDS

        let compute = cmds.iter().find(|c| c.name == "compute");
        assert!(compute.is_some());
        assert!(compute.unwrap().description.contains("Compute Engine"));
    }
}
