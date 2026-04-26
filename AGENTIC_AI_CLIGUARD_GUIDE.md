# cliguard for AI Agents

Auto-generate agentic AI guides from CLI help output

## Quick Reference

```bash
# Generate a Markdown guide for a CLI tool
cliguard cargo

# Force a specific framework parser
cliguard my-tool --framework clap
cliguard kubectl --framework cobra
cliguard pip --framework click
cliguard my-script --framework argparse
cliguard ls --framework gnu

# Skip recursive subcommand crawl (faster for large tools)
cliguard gcloud --no-recurse

# Output to a file
cliguard cargo --output cargo-guide.md

# Output JSON IR (for WisdomGuard)
cliguard cargo --format json --output cargo_ir.json

# Pipe JSON IR into WisdomGuard
cliguard cargo --format json | wisdomguard /dev/stdin --project my-gcp-project
```

## Global Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-f, --framework` | string | auto | Force a specific framework parser (clap, cobra, click, argparse, gnu) |
| `-o, --output` | string | stdout | Output file path (stdout if not specified) |
| `--format` | string | md | Output format: md or json [possible values: md, json] |
| `--no-recurse` | bool | false | Skip recursive subcommand help (faster, top-level commands only) |

---

## Common Workflows

### Generate a Markdown guide for a Rust CLI tool

```bash
# Auto-detects Clap framework from cargo's help output
cliguard cargo --output docs/cargo-guide.md

# Verify the output
wc -l docs/cargo-guide.md
head -50 docs/cargo-guide.md
```

### Generate an enhanced guide with WisdomGuard

```bash
# Step 1: produce the JSON IR
cliguard cargo --format json --output cargo_ir.json

# Step 2: enhance with Gemini (adds workflows, gotchas, key commands, error solutions)
wisdomguard cargo_ir.json \
  --project my-gcp-project \
  --output docs/cargo-guide-enhanced.md
```

### Generate a guide for kubectl and merge WisdomGuard enhancements

```bash
# Generate base guide
cliguard kubectl --output kubectl-base.md

# Generate IR
cliguard kubectl --format json --output kubectl_ir.json

# Merge enhancements into base guide
wisdomguard kubectl_ir.json \
  --base-guide kubectl-base.md \
  --project my-gcp-project \
  --output kubectl-enhanced.md
```

### Handle gcloud (very deep subcommand tree)

```bash
# Fast mode — top-level only (seconds)
cliguard gcloud --no-recurse --output gcloud-top.md

# Full recursive crawl — be prepared to wait several minutes
cliguard gcloud --output gcloud-full.md

# Full recursive + WisdomGuard (background-friendly)
cliguard gcloud --format json | wisdomguard /dev/stdin \
  --project my-gcp-project \
  --output gcloud-guide.md
```

### Generate guides for multiple tools in CI

```bash
TOOLS=(cargo kubectl docker gh)
for tool in "${TOOLS[@]}"; do
  cliguard "$tool" \
    --format json \
    --output "ir/${tool}.json"
done

for ir in ir/*.json; do
  name=$(basename "$ir" .json)
  wisdomguard "$ir" \
    --project "$GCP_PROJECT" \
    --output "docs/${name}-guide.md"
done
```

### Inspect the JSON IR to count subcommands

```bash
# Count top-level commands
cliguard cargo --format json | jq '.commands | length'

# List all command names
cliguard cargo --format json | jq '[.commands[].name]'

# Count total flags across all commands
cliguard cargo --format json | jq '[.commands[].flags[]] | length'
```

---

## Common Mistakes

| Wrong | Right | Why |
|-------|-------|-----|
| `cliguard gcloud` (no `--no-recurse`) for a quick overview | `cliguard gcloud --no-recurse` | gcloud has hundreds of subcommands; full recursion can take several minutes |
| `cliguard pip` without `--framework click` when auto-detect fails | `cliguard pip --framework click` | pip's help output is Click-generated but may not trigger the auto-detector if the binary is wrapped |
| `cliguard my-tool --format json > guide.md` | `cliguard my-tool > guide.md` | Without `--format json`, stdout is already Markdown; piping JSON to a `.md` file produces garbage |
| Running `cliguard` on a tool that requires sudo for `--help` | Pre-generate the help text and parse from a file, or run as the appropriate user | CliGuard runs the binary as the current user; if `--help` requires elevated permissions it will fail |
| Expecting CliGuard to parse man pages | CliGuard only uses `--help` / `-h` output | Man pages have a different structure; CliGuard does not invoke `man` |
| `--framework gnu` for a Python tool using argparse | `--framework argparse` | GNU parser misses `positional arguments:` sections; argparse parser handles them correctly |

---

## Key Commands

- `cliguard <binary>` — auto-detect framework, output Markdown guide to stdout
- `cliguard <binary> --output guide.md` — write guide to file
- `cliguard <binary> --format json` — emit JSON IR (for WisdomGuard)
- `cliguard <binary> --no-recurse` — top-level only, much faster for large tools
- `cliguard <binary> --framework cobra` — force framework, skip auto-detection
- `cliguard <binary> --format json | wisdomguard /dev/stdin --project <id>` — full pipeline, no temp file

---

## Error Messages

| Error / Exit Code | Meaning | Solution |
|-------------------|---------|---------|
| Exit code `1` | Binary not found or I/O error | Confirm the binary is on `PATH`; try `which <binary>` |
| Exit code `2` | Parse error — no recognisable help output | Run `<binary> --help` manually to check the output; use `--framework gnu` as a fallback |
| `No commands found` in IR | Tool has no subcommands | Expected for simple single-command tools; the IR will have an empty `commands` array |
| `Timeout after 10s` for a subcommand | A subcommand's `--help` hung | The subcommand may require args, a running service, or network access; use `--no-recurse` |
| Framework detected as `gnu` but flags are missing | GNU fallback is less precise than framework parsers | Force the correct parser with `--framework clap` / `--framework cobra` / etc. |
| Output file is empty | Binary exited non-zero on `--help` | Some tools use exit code 1 for `--help`; CliGuard still captures stdout, but check stderr output |
| `Permission denied` writing output | Output path is not writable | Check directory permissions; output file is written with mode `0o600` |

---
**Framework**: clap | **Version**: cliguard 0.1.0
