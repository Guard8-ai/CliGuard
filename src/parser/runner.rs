use anyhow::Result;
use regex::Regex;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;
use wait_timeout::ChildExt;

/// Maximum time to wait for a binary to produce help output.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum size of captured help output (10 MB).
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

static RE_ANSI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());

/// Strip ANSI escape codes from a string.
fn strip_ansi(s: &str) -> String {
    RE_ANSI.replace_all(s, "").into_owned()
}

/// Run a binary with the given arguments, enforcing a timeout and output size limit.
fn run_binary(binary: &Path, args: &[&str]) -> Result<String> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if child.wait_timeout(TIMEOUT)?.is_none() {
        // Timed out — kill the child and bail
        let _ = child.kill();
        let _ = child.wait();
        anyhow::bail!(
            "Binary timed out after {}s",
            TIMEOUT.as_secs()
        );
    }

    let output = child.wait_with_output()?;

    // Enforce size limit before converting to String
    if output.stdout.len() > MAX_OUTPUT_BYTES || output.stderr.len() > MAX_OUTPUT_BYTES {
        anyhow::bail!("Help output exceeds maximum size of {MAX_OUTPUT_BYTES} bytes");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Many tools print help to stderr on error, or to stdout on success
    // Strip ANSI escape codes (e.g. gcloud embeds color codes in help)
    if stdout.trim().is_empty() && !stderr.trim().is_empty() {
        Ok(strip_ansi(&stderr))
    } else {
        Ok(strip_ansi(&stdout))
    }
}

/// Get the top-level help output from a binary.
pub fn get_help(binary: &Path) -> Result<String> {
    // Try --help first, then -h, then help subcommand
    if let Ok(output) = run_binary(binary, &["--help"]) {
        if !output.trim().is_empty() {
            return Ok(output);
        }
    }
    if let Ok(output) = run_binary(binary, &["-h"]) {
        if !output.trim().is_empty() {
            return Ok(output);
        }
    }
    if let Ok(output) = run_binary(binary, &["help"]) {
        if !output.trim().is_empty() {
            return Ok(output);
        }
    }
    let name = binary.file_name().map_or("unknown", |n| n.to_str().unwrap_or("unknown"));
    anyhow::bail!("Could not get help output from: {name}")
}

/// Get help for a specific subcommand.
pub fn get_subcommand_help(binary: &Path, subcommand: &str) -> Option<String> {
    let parts: Vec<&str> = subcommand.split_whitespace().collect();

    // Try: <binary> <subcmd...> --help
    let mut args = parts.clone();
    args.push("--help");
    if let Ok(output) = run_binary(binary, &args) {
        if !output.trim().is_empty() {
            return Some(output);
        }
    }

    // Try: <binary> help <subcmd...>
    let mut args = vec!["help"];
    args.extend_from_slice(&parts);
    if let Ok(output) = run_binary(binary, &args) {
        if !output.trim().is_empty() {
            return Some(output);
        }
    }

    None
}

/// Get version string from a binary.
pub fn get_version(binary: &Path) -> Option<String> {
    if let Ok(output) = run_binary(binary, &["--version"]) {
        let trimmed = output.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}
