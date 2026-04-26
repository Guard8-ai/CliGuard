# Output Formats

CliGuard produces two output formats: Markdown (default) and JSON IR.

---

## Markdown Guide (`--format md`)

The default output. A structured Markdown document containing every command, subcommand, flag, and argument — ready for an AI agent to use.

### Sections

| Section | Content |
|---------|---------|
| `# <tool> for AI Agents` | Tool name, version, description |
| `## Quick Reference` | All commands in a compact bash block, grouped by command group |
| `## Command Reference` | Per-command (recursive): arguments table, options table, subcommand sections |
| `## Global Options` | Table of flags available to all commands |
| Footer | `**Framework**: <fw> \| **Version**: <version>` |

### Quick Reference Example

```markdown
## Quick Reference

```bash
# Core Commands
cargo build [OPTIONS]         # Compile the current package
cargo test [OPTIONS]          # Run the tests
cargo run [OPTIONS] [-- ARGS] # Run a binary

# Package Commands
cargo add <DEP>               # Add a dependency
cargo remove <DEP>            # Remove a dependency
```
```

### Command Reference Example

```markdown
## Command Reference

### `cargo build`
Compile the current package.

**Arguments**

| Argument | Description | Required |
|----------|-------------|----------|
| `[PACKAGE]` | Package to build | No |

**Options**

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--release` | `-r` | Build with optimizations | false |
| `--target <TRIPLE>` | | Build for the target triple | host |
| `--features <FEATURES>` | | Space-separated list of features | |
```

---

## JSON IR (`--format json`)

The structured Intermediate Representation. Used as input for WisdomGuard enhancement.

### Top-Level Schema

```json
{
  "name": "cargo",
  "version": "1.82.0",
  "description": "Rust's package manager",
  "framework": "Clap",
  "global_flags": [...],
  "commands": [...],
  "groups": [...]
}
```

### `commands` Array (recursive)

```json
{
  "name": "build",
  "description": "Compile the current package",
  "aliases": ["b"],
  "flags": [
    {
      "short": "r",
      "long": "release",
      "description": "Build with optimizations",
      "value_type": "Bool",
      "required": false,
      "default": null,
      "env_var": null
    },
    {
      "short": null,
      "long": "target",
      "description": "Build for the target triple",
      "value_type": "String",
      "required": false,
      "default": null,
      "env_var": "CARGO_BUILD_TARGET"
    }
  ],
  "args": [
    {
      "name": "PACKAGE",
      "description": "Package to build",
      "required": false,
      "value_type": "String"
    }
  ],
  "examples": [],
  "subcommands": []
}
```

### `value_type` Values

| Value | Meaning |
|-------|---------|
| `Bool` | Boolean flag (no value) |
| `String` | Free-form string |
| `Int` | Integer |
| `Float` | Floating point |
| `Path` | File or directory path |
| `Enum` | One of a fixed set of values |
| `Custom` | Framework-specific type annotation |

### `groups` Array

```json
[
  {
    "name": "Build Commands",
    "commands": ["build", "check", "clean"]
  },
  {
    "name": "Package Commands",
    "commands": ["add", "remove", "update"]
  }
]
```

Groups are used to organize the Quick Reference section in Markdown output.

---

## Output File Permissions

When `--output` is specified, the file is written with mode `0o600`. Writes to system directories and paths with `..` are blocked.
