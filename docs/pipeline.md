# CliGuard in the Guard8.ai Pipeline

CliGuard is the first stage of the Guard8.ai tool chain for CLI tools. It introspects a binary via `--help` recursion and produces a structured JSON IR that WisdomGuard can enhance with LLM-generated insights.

---

## Pipeline Overview

```
CLI Binary             CliGuard            WisdomGuard
(any installed    ──────────────►  JSON IR  ──────────────►  Enhanced Markdown
 tool)                             (ToolSpec)                 Guide
```

1. **CliGuard** runs `<binary> --help` recursively → produces a JSON IR (`ToolSpec`) or a Markdown guide
2. **WisdomGuard** reads the JSON IR → calls VertexAI Gemini → produces enriched Markdown

---

## Step-by-Step Example

### Step 1 — Generate the JSON IR

```bash
cliguard cargo --format json --output cargo_ir.json
```

`cargo_ir.json` now contains the `ToolSpec` structure:

```json
{
  "name": "cargo",
  "version": "1.82.0",
  "framework": "Clap",
  "commands": [...],
  "groups": [...]
}
```

### Step 2 — Enhance with WisdomGuard

```bash
wisdomguard cargo_ir.json \
  --project my-gcp-project \
  --output cargo-guide.md
```

`cargo-guide.md` contains:
- **Common Workflows** — multi-step bash recipes for realistic tasks
- **Common Mistakes** — wrong/right/why table for this specific tool
- **Key Commands** — top 20% of commands covering 80% of use cases
- **Error Messages** — common error strings mapped to solutions

### Step 3 (optional) — Merge into an existing Markdown guide

If you want to inject enhancements into an existing CliGuard Markdown output:

```bash
# First generate the base guide
cliguard cargo --output cargo-base.md

# Then enrich it (uses the JSON IR for the LLM, merges into the base)
cliguard cargo --format json | wisdomguard /dev/stdin \
  --base-guide cargo-base.md \
  --project my-gcp-project \
  --output cargo-enhanced.md
```

---

## One-Liner Pipe

```bash
cliguard kubectl --format json | wisdomguard /dev/stdin --project my-gcp-project
```

---

## CI / Automation Example

```yaml
# .github/workflows/docs.yml
- name: Generate tool guides
  run: |
    cliguard cargo --format json --output cargo_ir.json
    wisdomguard cargo_ir.json \
      --project ${{ vars.GCP_PROJECT }} \
      --output docs/cargo-guide.md
  env:
    GOOGLE_APPLICATION_CREDENTIALS: ${{ secrets.GCP_SA_KEY_PATH }}
```

---

## Skipping WisdomGuard

CliGuard produces a complete Markdown guide on its own:

```bash
# Full Markdown guide, no LLM needed
cliguard cargo --output cargo-guide.md
```

The standalone guide contains every command, subcommand, flag, and argument. Useful when you do not have a GCP project or need a fast, deterministic, cost-free output.

---

## Handling Large Tools

Some tools (like `gcloud`) have hundreds of subcommands. Use `--no-recurse` for speed:

```bash
# Fast: top-level only (seconds)
cliguard gcloud --no-recurse --format json | wisdomguard /dev/stdin --project my-project

# Full: recursive crawl (can take several minutes for gcloud)
cliguard gcloud --format json | wisdomguard /dev/stdin --project my-project
```

---

## Output Comparison

| Mode | Command | Contains |
|------|---------|---------|
| Markdown only | `cliguard <binary>` | Command/subcommand reference, flags, Quick Reference |
| JSON IR only | `cliguard <binary> --format json` | Machine-readable `ToolSpec` struct for tooling |
| Enhanced Markdown | `cliguard <binary> --format json \| wisdomguard /dev/stdin --project …` | All of above + Workflows, Gotchas, Key Commands, Errors |
| Merged | `wisdomguard ir.json --base-guide guide.md --project …` | Existing guide with enhancements injected at correct positions |

---

## Related

- [Output Formats](output.md) — full `ToolSpec` JSON schema
- [Supported Frameworks](frameworks.md) — which CLI frameworks CliGuard detects
- [WisdomGuard docs](../WisdomGuard/docs/index.md) — full WisdomGuard reference
