# CliGuard

**Auto-generate agentic AI guides from CLI tool help output.**

CliGuard introspects any installed binary by running its `--help` output (recursively across all subcommands) and produces a structured Markdown guide — or a JSON IR — purpose-built for AI agents to consume.

---

## What It Does

| Input | Output |
|-------|--------|
| Any CLI binary on `PATH` | Markdown guide with all commands, flags, and examples |
| Absolute or relative binary path | JSON IR for WisdomGuard enhancement |

CliGuard supports all major CLI frameworks: Clap, Cobra, Click, Argparse, gcloud, and GNU-style tools. It recursively discovers every subcommand, flag, and argument — no manual spec writing required.

---

## Quick Start

```bash
# Install
cargo install --path .

# Generate a guide for any CLI tool
cliguard cargo

# Write to file
cliguard kubectl -o AGENTIC_AI_KUBECTL_GUIDE.md

# Fast mode — top-level only, no recursion
cliguard gh --no-recurse

# Full pipeline with WisdomGuard enhancement
cliguard cargo -o guide.md
cliguard cargo --format json -o ir.json
wisdomguard ir.json --base-guide guide.md -o AGENTIC_AI_CARGO_GUIDE.md
```

---

## Navigation

- [Installation](installation.md)
- [Usage & CLI Reference](usage.md)
- [Supported Frameworks](frameworks.md)
- [Output Formats & IR Schema](output.md)
- [Pipeline Integration](pipeline.md)
