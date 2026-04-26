# Supported Frameworks

CliGuard auto-detects the CLI framework from the help output. Use `--framework` to force a specific parser.

---

## Detection Order

CliGuard tests parsers in this priority order, using the first match:

1. **gcloud** — highest priority (specific format)
2. **Cobra** — `Available Commands:` + `Flags:`, or uppercase `CORE COMMANDS` + `FLAGS`
3. **Clap** — `Usage:` + `Options:` + `-h, --help`
4. **Click** — `Show this message and exit.`
5. **Argparse** — `usage:` (lowercase) + `positional arguments:`
6. **GNU** — fallback for any `Usage:` + `--` style output

---

## Clap

**`--framework` value:** `clap`  
**Example tools:** `cargo`, `rg` (ripgrep), `fd`, `bat`, most Rust CLIs

Clap is the dominant Rust CLI framework. Help output has `Usage:` / `Options:` sections with short and long flags aligned in columns.

```bash
cliguard cargo
cliguard rg --framework clap
cliguard fd
```

**Parsed elements:** subcommands, short+long flags, required vs optional args, value types, default values, help descriptions.

---

## Cobra

**`--framework` value:** `cobra`  
**Example tools:** `kubectl`, `docker`, `hugo`, `helm`, `gh`

Cobra is the dominant Go CLI framework. Two variants are supported:

- **Standard Cobra:** `Available Commands:` section + `Flags:` section
- **gh-style Cobra:** Uppercase `CORE COMMANDS` / `COMMANDS` + `FLAGS` sections (used by GitHub CLI and similar tools)

```bash
cliguard kubectl
cliguard docker --framework cobra
cliguard gh
```

**Parsed elements:** command groups (CORE COMMANDS, ADDITIONAL COMMANDS), subcommand aliases, persistent flags vs local flags.

---

## gcloud

**Detection:** auto-detect only (not selectable via `--framework`)  
**Example tools:** `gcloud`

Google Cloud CLI uses a unique format with `NAME`, `SYNOPSIS`, `DESCRIPTION`, `GLOBAL FLAGS`, and `GROUPS` / `COMMANDS` sections (all uppercase headers). CliGuard has a dedicated parser for this format.

```bash
cliguard gcloud
cliguard gcloud --no-recurse    # gcloud has hundreds of subcommands; fast mode recommended
```

> `gcloud` has a very deep subcommand tree. Use `--no-recurse` for a fast top-level guide, or be prepared to wait several minutes for a full recursive crawl.

---

## Click

**`--framework` value:** `click`  
**Example tools:** `pip`, `flask`, `black`, `pytest`, `alembic`

Click is the dominant Python CLI framework. Help output contains `Show this message and exit.` and uses two-space indentation for option descriptions.

```bash
cliguard pip --framework click
cliguard flask
cliguard black
```

**Parsed elements:** commands and subcommands, options with type annotations (TEXT, INTEGER, PATH, etc.), environment variable bindings.

---

## Argparse

**`--framework` value:** `argparse`  
**Example tools:** Most Python scripts using `argparse`

Argparse (Python standard library) uses lowercase `usage:` and a `positional arguments:` section, distinguishing it from GNU-style output.

```bash
cliguard my-python-tool --framework argparse
```

**Parsed elements:** positional arguments, optional arguments (`-f`/`--flag`), metavar names, default values from help text.

---

## GNU

**`--framework` value:** `gnu`  
**Example tools:** `ls`, `grep`, `curl`, `tar`, `find`, `sed`

The fallback parser. Handles any `Usage:` header followed by `--flag` style options. Less structured than framework-specific parsers but covers the vast majority of C and shell CLIs.

```bash
cliguard ls
cliguard grep --framework gnu
cliguard curl
```

**Parsed elements:** flags (short and long), brief descriptions. Subcommand recursion is limited as GNU tools rarely have subcommands.

---

## Recursion

For tools with many subcommands, CliGuard recursively fetches help text for each discovered command. Limits:

| Limit | Value |
|-------|-------|
| Max depth | 10 levels |
| Max total commands | 5,000 |
| Timeout per subprocess | 10 seconds |

Use `--no-recurse` to skip recursion and only parse the top-level help.
