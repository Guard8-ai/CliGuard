# CliGuard

Auto-generate agentic AI guides from CLI tool help output.

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Auto-detect framework, output markdown
cliguard cargo

# Force specific framework
cliguard gh --framework cobra

# JSON IR output (for WisdomGuard pipeline)
cliguard cargo --format json -o cargo-ir.json

# Write to file
cliguard gcloud -o AGENTIC_AI_GCLOUD_GUIDE.md

# Skip recursive subcommand help (fast mode)
cliguard gh --no-recurse
```

## Supported Frameworks

| Framework | Detection | Examples |
|-----------|-----------|----------|
| Clap | `Usage:` + `Options:` + `-h, --help` | cargo, ripgrep, clap-based tools |
| Cobra | `Available Commands:` + `Flags:` | kubectl, docker, hugo |
| Cobra (gh-style) | `CORE COMMANDS` + `FLAGS` (uppercase) | gh |
| gcloud | `NAME` + `SYNOPSIS` + `GLOBAL FLAGS` | gcloud |
| Click | `Show this message and exit.` | pip, flask, black |
| Argparse | `usage:` + `positional arguments:` | python CLI tools |
| GNU | `Usage:` + `--` options (fallback) | ls, grep, curl |

## Output Formats

- **Markdown** (`--format md`, default) — ready-to-use agentic guide
- **JSON** (`--format json`) — structured IR for WisdomGuard enhancement

## Pipeline

```bash
# Generate base guide + enhance with VertexAI
cliguard cargo -o cargo-guide.md
cliguard cargo --format json -o cargo-ir.json
wisdomguard cargo-ir.json --base-guide cargo-guide.md -o AGENTIC_AI_CARGO_GUIDE.md
```

## License

MIT — Guard8.ai
