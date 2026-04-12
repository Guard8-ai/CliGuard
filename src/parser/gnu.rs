use crate::ir::{Arg, Command, Flag, Framework, ToolSpec, ValueType};
use crate::parser::Parser;
use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;

static RE_VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d+\.\d+(?:\.\d+)?(?:-\w+)?)\b").unwrap());

static RE_GNU_FLAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(-\w)(?:\s*,\s*)?)?(?:(--[\w-]+))?(?:\[?=(\w+)\]?)?\s{2,}(.+)$").unwrap()
});

static RE_USAGE_ARG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([A-Z][A-Z_]*)\](?:\.\.\.)?").unwrap());

pub struct GnuParser;

impl Parser for GnuParser {
    fn name(&self) -> &'static str {
        "gnu"
    }

    fn framework(&self) -> Framework {
        Framework::GnuStyle
    }

    fn detect(&self, help_output: &str) -> bool {
        // GNU-style: has "Usage:" or "usage:", options start with leading dashes,
        // and typically shows "--help" and "--version"
        // This is the fallback parser, so detection is broad
        (help_output.contains("Usage:") || help_output.contains("usage:"))
            && help_output.contains("--")
    }

    fn parse(
        &self,
        binary_name: &str,
        help_output: &str,
        _sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        let description = parse_description(help_output);
        let global_flags = parse_gnu_options(help_output);
        let args = parse_usage_args(help_output);

        let commands = if args.is_empty() {
            Vec::new()
        } else {
            vec![Command {
                name: binary_name.to_string(),
                description: description.clone(),
                aliases: Vec::new(),
                subcommands: Vec::new(),
                flags: Vec::new(),
                args,
                examples: Vec::new(),
            }]
        };

        Ok(ToolSpec {
            name: binary_name.to_string(),
            version: parse_version(help_output),
            description,
            framework: Framework::GnuStyle,
            commands,
            global_flags,
            groups: Vec::new(),
        })
    }
}

fn parse_version(help_output: &str) -> Option<String> {
    for line in help_output.lines().take(3) {
        if let Some(m) = RE_VERSION.find(line) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

fn parse_description(help_output: &str) -> String {
    // GNU tools often have description after "Usage:" lines
    let mut lines = Vec::new();
    let mut past_usage = false;
    let mut usage_count = 0;

    for line in help_output.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("Usage:") || trimmed.starts_with("usage:") {
            past_usage = true;
            usage_count += 1;
            continue;
        }

        // Skip additional usage lines (some GNU tools show multiple)
        if past_usage && usage_count > 0 && trimmed.starts_with("or:") {
            continue;
        }

        if !past_usage {
            continue;
        }

        // Stop at options section
        if trimmed.starts_with('-') || trimmed.is_empty() && !lines.is_empty() {
            break;
        }

        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    if lines.is_empty() {
        // Try first non-empty line before Usage
        for line in help_output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Usage:") || trimmed.starts_with("usage:") {
                break;
            }
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    lines.join(" ")
}

fn parse_gnu_options(help_output: &str) -> Vec<Flag> {
    let mut flags = Vec::new();
    let mut current_flag_text = String::new();

    for line in help_output.lines() {
        let trimmed = line.trim();

        // GNU options start with - in the trimmed line, with leading whitespace in the raw line
        if trimmed.starts_with('-') && line.starts_with(' ') {
            // Save previous flag if any
            if !current_flag_text.is_empty() {
                if let Some(flag) = parse_gnu_flag(&current_flag_text) {
                    flags.push(flag);
                }
            }
            current_flag_text = trimmed.to_string();
        } else if !current_flag_text.is_empty()
            && !trimmed.is_empty()
            && line.starts_with(' ')
            && !trimmed.starts_with('-')
        {
            // Multi-line description continuation
            current_flag_text.push(' ');
            current_flag_text.push_str(trimmed);
        } else if !current_flag_text.is_empty() {
            if let Some(flag) = parse_gnu_flag(&current_flag_text) {
                flags.push(flag);
            }
            current_flag_text.clear();
        }
    }

    // Last flag
    if !current_flag_text.is_empty() {
        if let Some(flag) = parse_gnu_flag(&current_flag_text) {
            flags.push(flag);
        }
    }

    // Filter help/version
    flags
        .into_iter()
        .filter(|f| {
            f.long.as_deref() != Some("--help") && f.long.as_deref() != Some("--version")
        })
        .collect()
}

fn parse_gnu_flag(line: &str) -> Option<Flag> {
    // GNU patterns:
    //   -s, --long=ARG      Description
    //   -s, --long[=ARG]    Description (optional arg)
    //   --long=ARG          Description
    //   -s                  Description
    let caps = RE_GNU_FLAG.captures(line)?;
    let short = caps.get(1).map(|m| m.as_str().to_string());
    let long = caps.get(2).map(|m| m.as_str().to_string());
    let arg_name = caps.get(3).map(|m| m.as_str().to_string());
    let description = caps[4].trim().to_string();

    if short.is_none() && long.is_none() {
        return None;
    }

    let value_type = arg_name
        .as_deref()
        .map_or(ValueType::Bool, infer_gnu_type);

    // Optional arg if it was in brackets
    let required = arg_name.is_some() && !line.contains("[=");

    Some(Flag {
        short,
        long,
        description,
        value_type,
        required,
        default: None,
        env_var: None,
    })
}

fn parse_usage_args(help_output: &str) -> Vec<Arg> {
    // GNU usage: "Usage: tool [OPTION]... [FILE]..."
    let mut args = Vec::new();
    for line in help_output.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Usage:") && !trimmed.starts_with("usage:") {
            continue;
        }
        // Extract bracketed args after [OPTION]...
        for cap in RE_USAGE_ARG.captures_iter(trimmed) {
            let name = &cap[1];
            if name == "OPTION" || name == "OPTIONS" || name.is_empty() {
                continue;
            }
            args.push(Arg {
                name: name.to_string(),
                description: String::new(),
                value_type: infer_gnu_arg_type(name),
                required: false,
                variadic: cap[0].contains("..."),
            });
        }
        break;
    }
    args
}

fn infer_gnu_type(name: &str) -> ValueType {
    match name.to_uppercase().as_str() {
        "FILE" | "PATH" | "DIR" | "DIRECTORY" => ValueType::Path,
        "NUM" | "N" | "NUMBER" | "COUNT" | "SIZE" | "BYTES" | "WIDTH" | "COLS" => ValueType::Int,
        _ => ValueType::String,
    }
}

#[allow(clippy::match_same_arms)]
fn infer_gnu_arg_type(name: &str) -> ValueType {
    match name {
        "FILE" | "FILES" | "SOURCE" | "DEST" | "DIRECTORY" => ValueType::Path,
        "PATTERN" | "EXPRESSION" => ValueType::String,
        _ => ValueType::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_GNU_HELP: &str = "\
Usage: ls [OPTION]... [FILE]...
List information about the FILEs.

  -a, --all                  do not ignore entries starting with .
  -l                         use a long listing format
  -h, --human-readable       with -l, print sizes in human readable format
  -r, --reverse              reverse order while sorting
  -S                         sort by file size, largest first
  -t                         sort by time, newest first
      --color=WHEN           colorize the output; WHEN can be 'always', 'auto', or 'never'
      --help     display this help and exit
      --version  output version information and exit
";

    #[test]
    fn detects_gnu_output() {
        let parser = GnuParser;
        assert!(parser.detect(SAMPLE_GNU_HELP));
    }

    #[test]
    fn parses_description() {
        let desc = parse_description(SAMPLE_GNU_HELP);
        assert!(desc.contains("List information about the FILEs"));
    }

    #[test]
    fn parses_gnu_options() {
        let flags = parse_gnu_options(SAMPLE_GNU_HELP);
        // --help and --version filtered
        assert!(flags.len() >= 5);
        assert_eq!(flags[0].short, Some("-a".to_string()));
        assert_eq!(flags[0].long, Some("--all".to_string()));
        assert_eq!(flags[0].value_type, ValueType::Bool);
    }

    #[test]
    fn parses_color_flag_with_value() {
        let flags = parse_gnu_options(SAMPLE_GNU_HELP);
        let color = flags.iter().find(|f| f.long.as_deref() == Some("--color"));
        assert!(color.is_some());
        assert_eq!(color.unwrap().value_type, ValueType::String);
    }

    #[test]
    fn parses_usage_args() {
        let args = parse_usage_args(SAMPLE_GNU_HELP);
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "FILE");
        assert!(args[0].variadic);
    }
}
