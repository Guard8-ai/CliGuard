# cliguard for AI Agents

Auto-generate agentic AI guides from CLI help output

## Quick Reference

```bash
# Global options
cliguard --framework <value>                # Force a specific framework parser (clap, cobra,...
cliguard --output <value>                   # Output file path (stdout if not specified)
cliguard --format <value>                   # Output format: md or json [default: md] [possib...
cliguard --no-recurse                       # Skip recursive subcommand help (faster, top-lev...
```

## Global Options

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-f, --framework` | string | - | Force a specific framework parser (clap, cobra, click, argparse, gnu) |
| `-o, --output` | string | - | Output file path (stdout if not specified) |
| `--format` | string | md | Output format: md or json [default: md] [possible values: md, json] |
| `--no-recurse` | bool | - | Skip recursive subcommand help (faster, top-level commands only) |

---
**Framework**: clap | **Version**: cliguard 0.1.0
