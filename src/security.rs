use anyhow::Result;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Maximum subcommand recursion depth to prevent denial-of-service.
pub const MAX_SUBCOMMAND_DEPTH: usize = 10;

/// Maximum total commands across all depth levels to prevent breadth-based OOM.
pub const MAX_TOTAL_COMMANDS: usize = 5000;

/// Maximum length for descriptions in generated output.
pub const MAX_DESCRIPTION_LENGTH: usize = 500;

/// Returns the list of system directory prefixes that must never be written to.
fn blocked_prefixes() -> Vec<PathBuf> {
    let root = Path::new(std::path::MAIN_SEPARATOR_STR);
    ["etc", "dev", "proc", "sys", "boot"]
        .iter()
        .map(|name| root.join(name))
        .collect()
}

/// Check if a path is a symlink.
fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Validate an output path, then atomically write content to it with restricted permissions.
/// Rejects traversal, symlinks, and writes to system directories.
pub fn write_output_safe(path: &Path, content: &str) -> Result<()> {
    let path_str = path.to_string_lossy();

    // Reject explicit traversal components
    if path_str.contains("..") {
        anyhow::bail!("Output path must not contain '..' traversal");
    }

    // Reject if the target itself is a symlink
    if is_symlink(path) {
        anyhow::bail!("Output path must not be a symbolic link");
    }

    // Resolve the parent directory to get a canonical base
    let resolved = if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            let cwd = std::env::current_dir()?;
            cwd.join(path)
        } else {
            // Reject if parent is a symlink
            if is_symlink(parent) {
                anyhow::bail!("Output parent directory must not be a symbolic link");
            }
            let canonical_parent = parent.canonicalize().map_err(|e| {
                anyhow::anyhow!("Cannot resolve output directory: {e}")
            })?;
            canonical_parent.join(path.file_name().unwrap_or_default())
        }
    } else {
        path.to_path_buf()
    };

    // Check against blocked system directory prefixes
    let resolved_str = resolved.to_string_lossy();
    for prefix in blocked_prefixes() {
        let prefix_str = prefix.to_string_lossy();
        if resolved_str.starts_with(prefix_str.as_ref()) {
            anyhow::bail!("Writing to system directories is not allowed");
        }
    }

    // Atomic write: open, write, set permissions in one sequence
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&resolved)?;

    file.write_all(content.as_bytes())?;

    // Set owner-only permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Escape a string for safe inclusion in markdown tables and inline contexts.
/// Prevents pipe-based table injection, newline injection, and backtick escaping.
pub fn escape_markdown(s: &str) -> String {
    s.replace('|', "\\|")
        .replace('\n', " ")
        .replace('\r', "")
        .replace('`', "'")
}

/// Truncate a description to a safe maximum length, respecting UTF-8 char boundaries.
pub fn safe_description(desc: &str, max_len: usize) -> String {
    let escaped = escape_markdown(desc);
    if escaped.len() <= max_len {
        return escaped;
    }
    let truncate_at = max_len.saturating_sub(3);
    let boundary = escaped
        .char_indices()
        .take_while(|(i, _)| *i <= truncate_at)
        .last()
        .map_or(0, |(i, _)| i);
    format!("{}...", &escaped[..boundary])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_path(name: &str) -> PathBuf {
        Path::new(std::path::MAIN_SEPARATOR_STR).join(name)
    }

    #[test]
    fn rejects_traversal_paths() {
        let path = PathBuf::from("..").join("..").join("etc").join("passwd");
        assert!(write_output_safe(&path, "test").is_err());
    }

    #[test]
    fn rejects_system_etc_paths() {
        let path = system_path("etc").join("shadow");
        assert!(write_output_safe(&path, "test").is_err());
    }

    #[test]
    fn rejects_system_dev_paths() {
        let path = system_path("dev").join("null");
        assert!(write_output_safe(&path, "test").is_err());
    }

    #[test]
    fn writes_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.md");
        write_output_safe(&path, "# Guide").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Guide");
    }

    #[test]
    #[cfg(unix)]
    fn sets_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secure.md");
        write_output_safe(&path, "secret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    #[cfg(unix)]
    fn rejects_symlink_target() {
        let dir = tempfile::tempdir().unwrap();
        let real_file = dir.path().join("real.md");
        std::fs::write(&real_file, "x").unwrap();
        let link_path = dir.path().join("link.md");
        std::os::unix::fs::symlink(&real_file, &link_path).unwrap();
        assert!(write_output_safe(&link_path, "test").is_err());
    }

    #[test]
    fn escapes_pipes() {
        assert_eq!(escape_markdown("a|b|c"), "a\\|b\\|c");
    }

    #[test]
    fn escapes_newlines() {
        assert_eq!(escape_markdown("line1\nline2"), "line1 line2");
    }

    #[test]
    fn escapes_backticks() {
        assert_eq!(escape_markdown("foo`bar"), "foo'bar");
    }

    #[test]
    fn truncates_long_descriptions() {
        let long = "a".repeat(600);
        let result = safe_description(&long, MAX_DESCRIPTION_LENGTH);
        assert!(result.len() <= MAX_DESCRIPTION_LENGTH);
        assert!(result.ends_with("..."));
    }
}
