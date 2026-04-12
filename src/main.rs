mod generator;
mod ir;
mod parser;
mod security;

use crate::ir::Framework;
use crate::parser::runner;
use crate::parser::ParserRegistry;
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cliguard", version, about = "Auto-generate agentic AI guides from CLI help output")]
struct Cli {
    /// The CLI binary to generate a guide for
    binary: String,

    /// Force a specific framework parser (clap, cobra, click, argparse, gnu)
    #[arg(short, long)]
    framework: Option<String>,

    /// Output file path (stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format: md or json
    #[arg(long, default_value = "md")]
    format: OutputFormat,

    /// Skip recursive subcommand help (faster, top-level commands only)
    #[arg(long)]
    no_recurse: bool,
}

#[derive(Clone, clap::ValueEnum)]
enum OutputFormat {
    Md,
    Json,
}

fn parse_framework(name: &str) -> Result<Framework> {
    match name.to_lowercase().as_str() {
        "clap" => Ok(Framework::Clap),
        "cobra" => Ok(Framework::Cobra),
        "click" => Ok(Framework::Click),
        "argparse" => Ok(Framework::Argparse),
        "gnu" => Ok(Framework::GnuStyle),
        other => anyhow::bail!("Unknown framework: {other}. Supported: clap, cobra, click, argparse, gnu"),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Resolve binary path
    let binary_path = ParserRegistry::resolve_binary(&cli.binary)
        .context("Could not find binary")?;

    // Get help output
    let help_output = runner::get_help(&binary_path)
        .context("Could not get help output from binary")?;

    // Get version (used to supplement parsed spec)
    let detected_version = runner::get_version(&binary_path);

    // Set up subcommand help fetcher with breadth limit
    let binary_for_closure = binary_path.clone();
    let command_count = std::sync::atomic::AtomicUsize::new(0);
    let no_recurse = cli.no_recurse;
    let sub_help = move |subcmd: &str| -> Option<String> {
        if no_recurse {
            return None;
        }
        let count = command_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count >= security::MAX_TOTAL_COMMANDS {
            return None;
        }
        runner::get_subcommand_help(&binary_for_closure, subcmd)
    };

    let registry = ParserRegistry::new();

    // Parse
    let binary_name = binary_path
        .file_stem()
        .map_or_else(|| cli.binary.clone(), |s| s.to_string_lossy().into_owned());

    let mut spec = if let Some(ref fw_name) = cli.framework {
        let framework = parse_framework(fw_name)?;
        registry.parse_with_framework(&framework, &binary_name, &help_output, &sub_help)?
    } else {
        registry.detect_and_parse(&binary_name, &help_output, &sub_help)?
    };

    // Supplement version from --version output if parser didn't find one
    if spec.version.is_none() {
        spec.version = detected_version;
    }

    // Generate output
    let output = match cli.format {
        OutputFormat::Json => serde_json::to_string_pretty(&spec)
            .context("Failed to serialize to JSON")?,
        OutputFormat::Md => generator::generate_guide(&spec)
            .context("Failed to generate guide")?,
    };

    // Write output
    if let Some(ref path) = cli.output {
        security::write_output_safe(path, &output)
            .context("Failed to write output file")?;
        eprintln!("Guide written to output file");
    } else {
        print!("{output}");
    }

    Ok(())
}
