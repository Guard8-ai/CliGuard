# Usage & CLI Reference

## Synopsis

```
cliguard <binary> [OPTIONS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<binary>` | Name of the CLI tool on `PATH`, or an absolute/relative path to the binary |

## Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--framework <FW>` | `-f` | auto-detect | Force CLI framework parser (see below) |
| `--output <FILE>` | `-o` | stdout | Write output to file instead of stdout |
| `--format <FMT>` | — | `md` | Output format: `md` (Markdown) or `json` (IR) |
| `--no-recurse` | — | false | Skip recursive subcommand discovery (top-level only) |
| `--help` | `-h` | — | Print help |
| `--version` | `-V` | — | Print version |

### `--framework` values

| Value | Framework | Example tools |
|-------|-----------|---------------|
| `clap` | Clap (Rust) | cargo, ripgrep, fd |
| `cobra` | Cobra (Go) / gh-style | kubectl, docker, hugo, gh |
| `click` | Click (Python) | pip, flask, black, pytest |
| `argparse` | Argparse (Python) | many Python CLIs |
| `gnu` | GNU-style fallback | ls, grep, curl, tar |

> **Note:** The `gcloud` parser is auto-detected only — it cannot be forced via `--framework`. Use `cliguard gcloud` without `--framework` to trigger it.

## Examples

```bash
# Auto-detect framework, print Markdown to stdout
cliguard cargo

# Write guide to file
cliguard kubectl -o AGENTIC_AI_KUBECTL_GUIDE.md

# Force Cobra parser for a gh-style tool
cliguard gh --framework cobra

# Fast mode — top-level help only, no subcommand recursion
cliguard docker --no-recurse

# Generate JSON IR for WisdomGuard
cliguard cargo --format json -o cargo-ir.json

# Use an absolute path to a binary
cliguard /usr/local/bin/my-tool
```

## Recursion Limits

| Limit | Value |
|-------|-------|
| Max subcommand depth | 10 levels |
| Max total commands | 5,000 |
| Subprocess timeout | 10 seconds per invocation |
| Max output per subprocess | 10 MB |

Use `--no-recurse` to skip recursion entirely when you only need the top-level commands.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error (binary not found, parse failure, bad arguments) |
| `2` | File/security error (symlink output, path traversal, system directory) |

## Security Notes

- Binary is resolved via `which` (no shell expansion — no injection risk)
- Output files are written with mode `0o600`
- Writes to `/etc`, `/dev`, `/proc`, `/sys`, `/boot` are blocked
- `..` in output paths is rejected
- All descriptions are markdown-escaped in generated output
