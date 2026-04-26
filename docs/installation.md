# Installation

## Pre-built Binaries (recommended)

Download the latest release for your platform from the [releases page](https://github.com/Guard8-ai/CliGuard/releases):

| Platform | File |
|----------|------|
| Linux x86_64 | `cliguard-linux-x86_64` |
| macOS ARM64 | `cliguard-macos-aarch64` |
| Windows x86_64 | `cliguard-windows-x86_64.exe` |

```bash
# Linux / macOS
chmod +x cliguard-linux-x86_64
mv cliguard-linux-x86_64 ~/.local/bin/cliguard

# Verify
cliguard --version
```

## From Source

Requires Rust 1.80+.

```bash
git clone https://github.com/Guard8-ai/CliGuard
cd CliGuard
cargo install --path .
```

## Verify Installation

```bash
cliguard --version
cliguard --help
```
