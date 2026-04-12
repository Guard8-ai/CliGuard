pub mod argparse;
pub mod clap_parser;
pub mod click;
pub mod cobra;
pub mod gcloud;
pub mod gnu;
pub mod runner;

use crate::ir::{Framework, ToolSpec};
use anyhow::Result;
use std::path::Path;

/// Trait that each CLI framework parser implements.
pub trait Parser {
    /// Human-readable name of this parser.
    fn name(&self) -> &'static str;

    /// The framework this parser handles.
    fn framework(&self) -> Framework;

    /// Returns true if the help output appears to come from this framework.
    fn detect(&self, help_output: &str) -> bool;

    /// Parse the help output into a `ToolSpec`.
    fn parse(&self, binary_name: &str, help_output: &str, sub_help: &dyn Fn(&str) -> Option<String>) -> Result<ToolSpec>;
}

/// Registry of all available parsers, used for auto-detection.
pub struct ParserRegistry {
    parsers: Vec<Box<dyn Parser>>,
}

impl ParserRegistry {
    /// Create a registry with all built-in parsers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parsers: vec![
                Box::new(clap_parser::ClapParser),
                Box::new(gcloud::GcloudParser),
                Box::new(cobra::CobraParser),
                Box::new(click::ClickParser),
                Box::new(argparse::ArgparseParser),
                Box::new(gnu::GnuParser),
            ],
        }
    }

    /// Auto-detect framework from help output and parse.
    pub fn detect_and_parse(
        &self,
        binary_name: &str,
        help_output: &str,
        sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        for parser in &self.parsers {
            if parser.detect(help_output) {
                eprintln!("Detected framework: {}", parser.name());
                return parser.parse(binary_name, help_output, sub_help);
            }
        }
        // Fallback to GNU-style as it's the most generic
        let fallback = self.parsers.last()
            .ok_or_else(|| anyhow::anyhow!("No parsers registered"))?;
        eprintln!("No framework detected, falling back to {}", fallback.name());
        fallback.parse(binary_name, help_output, sub_help)
    }

    /// Parse using a specific framework.
    pub fn parse_with_framework(
        &self,
        framework: &Framework,
        binary_name: &str,
        help_output: &str,
        sub_help: &dyn Fn(&str) -> Option<String>,
    ) -> Result<ToolSpec> {
        for parser in &self.parsers {
            if &parser.framework() == framework {
                return parser.parse(binary_name, help_output, sub_help);
            }
        }
        anyhow::bail!("No parser registered for framework: {framework}")
    }

    /// Find a parser by path, checking if the binary exists.
    pub fn resolve_binary(binary: &str) -> Result<std::path::PathBuf> {
        if Path::new(binary).is_absolute() || binary.contains('/') {
            let path = Path::new(binary);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
            anyhow::bail!("Binary not found: {binary}");
        }
        which::which(binary).map_err(|_| anyhow::anyhow!("Binary not found on PATH: {binary}"))
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}
