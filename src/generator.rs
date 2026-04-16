use crate::ir::{Command, Flag, ToolSpec, ValueType};
use crate::security::{escape_markdown, safe_description, MAX_DESCRIPTION_LENGTH};
use std::fmt::Write;

/// Generate a markdown agentic AI guide from a `ToolSpec`.
///
/// # Errors
///
/// Returns an error if writing to the internal string buffer fails.
pub fn generate_guide(spec: &ToolSpec) -> anyhow::Result<String> {
    let mut out = String::with_capacity(4096);

    write_header(&mut out, spec)?;
    write_quick_reference(&mut out, spec)?;
    write_command_reference(&mut out, spec)?;
    write_global_flags(&mut out, spec)?;

    writeln!(out, "---")?;
    let version_str = spec.version.as_deref().unwrap_or("unknown");
    writeln!(
        out,
        "**Framework**: {} | **Version**: {}",
        spec.framework, version_str
    )?;

    Ok(out)
}

fn write_header(out: &mut String, spec: &ToolSpec) -> std::fmt::Result {
    writeln!(out, "# {} for AI Agents", escape_markdown(&spec.name))?;
    writeln!(out)?;
    if !spec.description.is_empty() {
        writeln!(
            out,
            "{}",
            safe_description(&spec.description, MAX_DESCRIPTION_LENGTH)
        )?;
        writeln!(out)?;
    }
    Ok(())
}

fn write_quick_reference(out: &mut String, spec: &ToolSpec) -> std::fmt::Result {
    writeln!(out, "## Quick Reference")?;
    writeln!(out)?;
    writeln!(out, "```bash")?;

    // Group commands by their group if available
    if spec.groups.is_empty() {
        for cmd in &spec.commands {
            write_quick_command(out, &spec.name, cmd, "")?;
        }
    } else {
        for group in &spec.groups {
            writeln!(out, "# {}", group.name)?;
            for cmd_name in &group.commands {
                if let Some(cmd) = spec.commands.iter().find(|c| &c.name == cmd_name) {
                    write_quick_command(out, &spec.name, cmd, "")?;
                }
            }
            writeln!(out)?;
        }
    }

    // Show key global flags
    if !spec.global_flags.is_empty() {
        writeln!(out, "# Global options")?;
        for flag in &spec.global_flags {
            let flag_str = format_flag_usage(flag);
            let padding = 45_usize.saturating_sub(spec.name.len() + flag_str.len() + 2);
            writeln!(
                out,
                "{} {}{}# {}",
                spec.name,
                flag_str,
                " ".repeat(padding),
                safe_description(&flag.description, 50)
            )?;
        }
    }

    writeln!(out, "```")?;
    writeln!(out)?;
    Ok(())
}

fn write_quick_command(
    out: &mut String,
    tool_name: &str,
    cmd: &Command,
    prefix: &str,
) -> std::fmt::Result {
    let full_name = if prefix.is_empty() {
        cmd.name.clone()
    } else {
        format!("{prefix} {}", cmd.name)
    };

    let args_str = cmd
        .args
        .iter()
        .map(|a| {
            if a.required {
                format!("<{}>", a.name)
            } else {
                format!("[{}]", a.name)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let line = if args_str.is_empty() {
        format!("{tool_name} {full_name}")
    } else {
        format!("{tool_name} {full_name} {args_str}")
    };

    let padding = 45_usize.saturating_sub(line.len());
    writeln!(
        out,
        "{}{}# {}",
        line,
        " ".repeat(padding),
        safe_description(&cmd.description, 50)
    )?;

    // Show subcommands indented
    for sub in &cmd.subcommands {
        write_quick_command(out, tool_name, sub, &full_name)?;
    }

    Ok(())
}

fn write_command_reference(out: &mut String, spec: &ToolSpec) -> std::fmt::Result {
    if spec.commands.is_empty() {
        return Ok(());
    }

    writeln!(out, "## Command Reference")?;
    writeln!(out)?;

    for cmd in &spec.commands {
        write_command_detail(out, &spec.name, cmd, 3)?;
    }

    Ok(())
}

fn write_command_detail(
    out: &mut String,
    tool_name: &str,
    cmd: &Command,
    heading_level: usize,
) -> std::fmt::Result {
    let heading = "#".repeat(heading_level);
    writeln!(out, "{heading} `{tool_name} {}`", cmd.name)?;
    writeln!(out)?;

    if !cmd.description.is_empty() {
        writeln!(
            out,
            "{}",
            safe_description(&cmd.description, MAX_DESCRIPTION_LENGTH)
        )?;
        writeln!(out)?;
    }

    // Arguments
    if !cmd.args.is_empty() {
        writeln!(out, "**Arguments:**")?;
        writeln!(out)?;
        writeln!(out, "| Name | Type | Required | Description |")?;
        writeln!(out, "|------|------|----------|-------------|")?;
        for arg in &cmd.args {
            writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                escape_markdown(&arg.name),
                format_value_type(&arg.value_type),
                if arg.required { "yes" } else { "no" },
                safe_description(&arg.description, MAX_DESCRIPTION_LENGTH)
            )?;
        }
        writeln!(out)?;
    }

    // Flags
    if !cmd.flags.is_empty() {
        writeln!(out, "**Options:**")?;
        writeln!(out)?;
        writeln!(out, "| Flag | Type | Default | Description |")?;
        writeln!(out, "|------|------|---------|-------------|")?;
        for flag in &cmd.flags {
            let flag_name = format_flag_name(flag);
            let default = flag.default.as_deref().unwrap_or("-");
            writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                escape_markdown(&flag_name),
                format_value_type(&flag.value_type),
                escape_markdown(default),
                safe_description(&flag.description, MAX_DESCRIPTION_LENGTH)
            )?;
        }
        writeln!(out)?;
    }

    // Subcommands
    for sub in &cmd.subcommands {
        let sub_tool = format!("{tool_name} {}", cmd.name);
        write_command_detail(out, &sub_tool, sub, heading_level + 1)?;
    }

    Ok(())
}

fn write_global_flags(out: &mut String, spec: &ToolSpec) -> std::fmt::Result {
    if spec.global_flags.is_empty() {
        return Ok(());
    }

    writeln!(out, "## Global Options")?;
    writeln!(out)?;
    writeln!(out, "| Flag | Type | Default | Description |")?;
    writeln!(out, "|------|------|---------|-------------|")?;

    for flag in &spec.global_flags {
        let flag_name = format_flag_name(flag);
        let default = flag.default.as_deref().unwrap_or("-");
        writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            escape_markdown(&flag_name),
            format_value_type(&flag.value_type),
            escape_markdown(default),
            safe_description(&flag.description, MAX_DESCRIPTION_LENGTH)
        )?;
    }

    writeln!(out)?;
    Ok(())
}

fn format_flag_name(flag: &Flag) -> String {
    match (&flag.short, &flag.long) {
        (Some(s), Some(l)) => format!("{s}, {l}"),
        (Some(s), None) => s.clone(),
        (None, Some(l)) => l.clone(),
        (None, None) => String::new(),
    }
}

fn format_flag_usage(flag: &Flag) -> String {
    let name = flag.long.as_deref().or(flag.short.as_deref()).unwrap_or("");
    if flag.value_type == ValueType::Bool {
        name.to_string()
    } else {
        format!("{name} <value>")
    }
}

fn format_value_type(vt: &ValueType) -> &str {
    match vt {
        ValueType::String => "string",
        ValueType::Bool => "bool",
        ValueType::Int => "int",
        ValueType::Float => "float",
        ValueType::Path => "path",
        ValueType::Enum(_) => "enum",
        ValueType::Custom(s) => s.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;

    fn sample_spec() -> ToolSpec {
        ToolSpec {
            name: "mytool".to_string(),
            version: Some("1.0.0".to_string()),
            description: "A sample tool for testing".to_string(),
            framework: Framework::Clap,
            commands: vec![
                Command {
                    name: "run".to_string(),
                    description: "Run the main process".to_string(),
                    aliases: Vec::new(),
                    subcommands: Vec::new(),
                    flags: vec![Flag {
                        short: Some("-n".to_string()),
                        long: Some("--count".to_string()),
                        description: "Number of iterations".to_string(),
                        value_type: ValueType::Int,
                        required: false,
                        default: Some("1".to_string()),
                        env_var: None,
                    }],
                    args: vec![Arg {
                        name: "INPUT".to_string(),
                        description: "Input file".to_string(),
                        value_type: ValueType::Path,
                        required: true,
                        variadic: false,
                    }],
                    examples: Vec::new(),
                },
                Command {
                    name: "check".to_string(),
                    description: "Validate configuration".to_string(),
                    aliases: Vec::new(),
                    subcommands: Vec::new(),
                    flags: Vec::new(),
                    args: Vec::new(),
                    examples: Vec::new(),
                },
            ],
            global_flags: vec![Flag {
                short: Some("-v".to_string()),
                long: Some("--verbose".to_string()),
                description: "Enable verbose output".to_string(),
                value_type: ValueType::Bool,
                required: false,
                default: None,
                env_var: None,
            }],
            groups: vec![CommandGroup {
                name: "Commands".to_string(),
                commands: vec!["run".to_string(), "check".to_string()],
            }],
        }
    }

    #[test]
    fn generates_non_empty_guide() {
        let guide = generate_guide(&sample_spec()).unwrap();
        assert!(!guide.is_empty());
    }

    #[test]
    fn guide_has_header() {
        let guide = generate_guide(&sample_spec()).unwrap();
        assert!(guide.contains("# mytool for AI Agents"));
    }

    #[test]
    fn guide_has_quick_reference() {
        let guide = generate_guide(&sample_spec()).unwrap();
        assert!(guide.contains("## Quick Reference"));
        assert!(guide.contains("```bash"));
        assert!(guide.contains("mytool run"));
    }

    #[test]
    fn guide_has_command_reference() {
        let guide = generate_guide(&sample_spec()).unwrap();
        assert!(guide.contains("## Command Reference"));
        assert!(guide.contains("`mytool run`"));
        assert!(guide.contains("| `INPUT`"));
    }

    #[test]
    fn guide_has_global_options() {
        let guide = generate_guide(&sample_spec()).unwrap();
        assert!(guide.contains("## Global Options"));
        assert!(guide.contains("--verbose"));
    }

    #[test]
    fn guide_has_footer() {
        let guide = generate_guide(&sample_spec()).unwrap();
        assert!(guide.contains("**Framework**: clap"));
        assert!(guide.contains("**Version**: 1.0.0"));
    }
}
