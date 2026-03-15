# Cabalist

[![CI](https://github.com/joshburgess/cabalist/actions/workflows/ci.yml/badge.svg)](https://github.com/joshburgess/cabalist/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

A strongly opinionated TUI tool for managing Haskell `.cabal` files, built in Rust with ratatui.

## Goal

Make pure-Cabal Haskell development as approachable as Stack, by providing a beautiful TUI that guides users through every pain point of `.cabal` file management.

## Installation

### Pre-built binaries (recommended)

Download the latest release for your platform from the [Releases page](https://github.com/joshburgess/cabalist/releases), or use the install script:

```sh
curl -fsSL https://raw.githubusercontent.com/joshburgess/cabalist/main/install.sh | bash
```

### Nix

```sh
nix run github:joshburgess/cabalist
```

Or add to your flake inputs:

```nix
{
  inputs.cabalist.url = "github:joshburgess/cabalist";
}
```

### Homebrew

```sh
brew install joshburgess/tap/cabalist
```

### Docker (CLI only)

```sh
docker run --rm -v "$PWD:/work" -w /work ghcr.io/joshburgess/cabalist-cli check
```

### From source

Requires Rust 1.75 or later:

```sh
git clone https://github.com/joshburgess/cabalist.git
cd cabalist
cargo install --path crates/cabalist-tui
cargo install --path crates/cabalist-cli
```

## Quick Start

```sh
# Launch the interactive TUI in your Haskell project directory
cabalist

# Or use the CLI for scripting and CI
cabalist-cli check          # Run opinionated lints
cabalist-cli add aeson      # Add a dependency with PVP bounds
cabalist-cli info           # Show project summary
```

## Features

- **Interactive dependency management** -- search Hackage, add/remove dependencies with proper PVP bounds
- **Extension browser** -- toggle GHC extensions with descriptions and safety notes
- **Opinionated lints** -- catch missing bounds, bad practices, and suggest improvements
- **Build integration** -- run `cabal build` and `cabal test` with streaming output
- **Round-trip fidelity** -- your comments, formatting, and field ordering are preserved
- **Project initialization** -- guided wizard to create well-structured `.cabal` files

## Architecture

Cabalist ships two binaries backed by shared library crates:

- **`cabalist`** -- the interactive TUI application
- **`cabalist-cli`** -- the scriptable, non-interactive CLI for automation and CI

### Crate structure

| Crate | Purpose |
|-------|---------|
| `cabalist-parser` | CST/AST parser for `.cabal` files with round-trip fidelity |
| `cabalist-project` | Parser for `cabal.project` files |
| `cabalist-ghc` | GHC extensions, warnings, and version knowledge base |
| `cabalist-hackage` | Hackage package index, search, and version bounds |
| `cabalist-opinions` | Opinionated lints, defaults, templates, and configuration |
| `cabalist-cabal` | Interface to the `cabal` CLI subprocess |
| `cabalist-tui` | The TUI application (ratatui) |
| `cabalist-cli` | The non-interactive CLI tool |

### Design principles

1. The `.cabal` file is the source of truth, always.
2. Preserve what the user wrote (comments, formatting, ordering).
3. Shell out to `cabal` for what it does well (solving, building).
4. Be strongly opinionated with transparent escape hatches.

## Building

```
cargo build --workspace
```

## Testing

```
cargo test --workspace
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
