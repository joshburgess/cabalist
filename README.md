<p align="center">
  <img src="logo.svg" alt="Cabalist" width="600"/>
</p>

<p align="center">
  <a href="https://github.com/joshburgess/cabalist/actions/workflows/ci.yml"><img src="https://github.com/joshburgess/cabalist/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg" alt="License: MIT OR Apache-2.0"></a>
</p>

**Cabalist** is a strongly opinionated toolkit for managing Haskell `.cabal` files. It provides an interactive TUI, a scriptable CLI, and an LSP server for editor integration — all backed by a parser with byte-identical round-trip fidelity.

The guiding principle: **make pure-Cabal Haskell development as approachable as Stack**, by eliminating the friction of `.cabal` file management while keeping the `.cabal` file as the source of truth.

## What Cabalist Does

- **Manages dependencies** — search Hackage, add packages with correct PVP bounds, detect outdated versions, visualize the dependency tree
- **Manages GHC extensions** — browse 200+ extensions with descriptions and safety notes, toggle them on/off
- **Lints your .cabal file** — 16 opinionated, individually configurable checks that catch missing bounds, bad practices, and structural issues
- **Formats your .cabal file** — round-trip safe formatting with optional alphabetical sorting
- **Builds your project** — run `cabal build`, `cabal test`, and `cabal clean` with streaming output and error navigation
- **Initializes new projects** — guided wizard with templates (library, application, library+exe, full)
- **Edits metadata** — change name, version, license, synopsis, and all other top-level fields
- **Manages cabal.project** — view and edit project-level configuration (compiler, index-state, constraints)
- **Integrates with editors** — full LSP server with diagnostics, completions, hover, code actions, formatting, semantic tokens, inlay hints, goto definition, and rename

All of this while **preserving your comments, formatting, and field ordering**. Cabalist never rewrites what you didn't ask it to change.

## Three Tools, One Codebase

| Tool | Use Case |
|------|----------|
| **`cabalist`** | Interactive TUI for day-to-day development |
| **`cabalist-cli`** | Scriptable CLI for automation, CI, and quick edits from the terminal |
| **`cabalist-lsp`** | LSP server for VS Code, Neovim, Emacs, and any LSP-compatible editor |

## Installation

### Pre-built Binaries

Download from the [Releases page](https://github.com/joshburgess/cabalist/releases).

### Homebrew (macOS / Linux)

```sh
brew install joshburgess/tap/cabalist
```

### Nix

```sh
nix run github:joshburgess/cabalist
```

### From Source

Requires Rust 1.75+:

```sh
cargo install --path crates/cabalist-tui   # Interactive TUI
cargo install --path crates/cabalist-cli   # CLI tool
cargo install --path crates/cabalist-lsp   # LSP server
```

## Quick Start

### TUI

```sh
cd my-haskell-project
cabalist                  # Launch the interactive TUI
```

Use `d` for dependencies, `e` for extensions, `b` for build, `m` for metadata, `p` for cabal.project. Press `?` for help anywhere.

### CLI

| Command | Description |
|---------|-------------|
| `cabalist-cli check` | Lint your `.cabal` file |
| `cabalist-cli add aeson` | Add a dependency with PVP bounds |
| `cabalist-cli add text --version "^>=2.0"` | Add with specific version constraint |
| `cabalist-cli remove old-package` | Remove a dependency |
| `cabalist-cli extensions --toggle DerivingStrategies` | Toggle an extension |
| `cabalist-cli set synopsis "My cool library"` | Set a metadata field |
| `cabalist-cli fmt` | Format the `.cabal` file |
| `cabalist-cli deps --outdated` | Check for outdated dependencies |
| `cabalist-cli deps --tree` | Show dependency tree |
| `cabalist-cli modules --scan` | Find `.hs` files not listed in `.cabal` |
| `cabalist-cli build` | Run `cabal build` |
| `cabalist-cli test` | Run `cabal test` |
| `cabalist-cli info` | Show project summary |
| `cabalist-cli init --type full` | Create a new project |
| `cabalist-cli update-index` | Download/refresh Hackage index |

### Editor (LSP)

Install `cabalist-lsp` and configure your editor:

- **VS Code**: See `editors/vscode/` for the extension
- **Neovim**: See `editors/neovim/init.lua`
- **Emacs**: See `editors/emacs/cabalist-lsp.el`

The LSP provides: diagnostics (16 lints), completions (packages, extensions, fields), hover documentation, code actions (quick fixes), document symbols (outline), formatting, semantic tokens, inlay hints (latest versions), goto definition (`import:` to `common` stanza), and rename (common stanza refactoring).

## Documentation

| Guide | What It Covers |
|-------|---------------|
| [Getting Started](docs/getting-started.md) | End-to-end walkthroughs for common scenarios |
| [TUI Guide](docs/tui-guide.md) | Every view, keybinding, and workflow in the interactive TUI |
| [CLI Reference](docs/cli-reference.md) | Every command, flag, and example for the CLI |
| [Editor Setup](docs/editor-setup.md) | VS Code, Neovim, and Emacs configuration |
| [Opinions & Lints](docs/opinions.md) | All 16 lints with rationale and configuration |
| [Configuration](docs/configuration.md) | `cabalist.toml` reference with all options |
| [Keybindings](docs/keybindings.md) | Complete TUI keyboard reference |
| [Cabal Spec Compliance](docs/cabal-spec-compliance.md) | Parser syntax support and test coverage |

## Architecture

Cabalist is a Rust workspace of 9 focused crates:

| Crate | Purpose |
|-------|---------|
| `cabalist-parser` | CST/AST parser with byte-identical round-trip fidelity |
| `cabalist-project` | Parser for `cabal.project` files |
| `cabalist-ghc` | GHC extensions (200+), warnings, and version knowledge base |
| `cabalist-hackage` | Hackage index, package search, PVP version bounds |
| `cabalist-opinions` | 16 lints, defaults, templates, and configuration |
| `cabalist-cabal` | Async subprocess interface to `cabal` CLI |
| `cabalist-tui` | Interactive TUI (ratatui + crossterm) |
| `cabalist-cli` | Non-interactive CLI (clap) |
| `cabalist-lsp` | LSP server (tower-lsp) |

### Design Principles

1. **The `.cabal` file is the source of truth** — always.
2. **Preserve what the user wrote** — comments, formatting, and field ordering are never lost.
3. **Shell out to `cabal` for what it does well** — solving, building, testing.
4. **Be strongly opinionated with transparent escape hatches** — every lint and default is individually configurable.

### Why Rust?

Cabalist is a tool for Haskell developers, written in Rust. This is a deliberate choice, not an accident.

**Zero-dependency installation.** Cabalist's goal is to make `.cabal` management as easy as Stack. Stack's magic is that it's a single binary you download and run. Cabalist achieves the same: `brew install cabalist` and you're done. A Haskell implementation would require a working GHC toolchain to build — creating a chicken-and-egg problem for the exact users (newcomers setting up their first project) that cabalist is designed to help.

**Trivial cross-platform static binaries.** Rust produces fully static, self-contained binaries for every major platform with a single `cargo build --target`. Our CI builds for Linux x64, macOS x64, macOS ARM, and Windows in a simple matrix. GHC can produce static binaries, but it requires musl libc on Linux, doesn't support fully static linking on macOS (Apple's linker requires dynamic system frameworks), and makes cross-compilation painful. For a tool whose purpose is lowering the barrier to entry, distribution friction matters.

**The parser needed to not be the Cabal library.** This sounds paradoxical, but: if you write a `.cabal` parser in Haskell, you'd naturally reach for `Cabal-syntax` — a 100k+ line library designed for evaluation and dependency solving, not round-trip editing. Cabalist's CST parser preserves every byte of the original source (comments, whitespace, field ordering) with byte-identical fidelity. This is a fundamentally different design than what `Cabal-syntax` provides, and building it from scratch in Rust made it natural to get the abstraction right without fighting an existing library's assumptions.

**TUI and LSP ecosystem maturity.** ratatui + crossterm is the most actively maintained TUI framework in any language, with excellent Windows terminal support. tower-lsp provides an async LSP server that starts in milliseconds — important for a tool that activates on every `.cabal` file open. Haskell's `brick` is excellent for TUIs but less actively developed, and Haskell LSP servers are known for slow startup due to GHC runtime initialization.

**Rust's type system fits the problem.** Ownership semantics naturally prevent use-after-free bugs in the CST arena (flat `Vec<Node>` with `NodeId` indices). No GC pressure on large parse trees. The async runtime (tokio) handles concurrent build subprocess streaming cleanly. These aren't things Haskell can't do — they're things Rust makes easy to get right by default.

**What Haskell would have been better at.** If cabalist needed to fully evaluate conditionals, resolve flags, or compute dependency solutions, the `Cabal` library would be the right tool. But cabalist intentionally doesn't do that — it shells out to `cabal` for solving and building, and works at the syntactic level where a purpose-built CST parser is the right abstraction.

The short version: cabalist is a developer tool, and the best developer tools minimize friction for their users. Rust minimizes friction at every point — installation, distribution, startup time, cross-platform support — in ways that directly serve cabalist's mission of making Haskell development more approachable.

## Contributing

```sh
cargo build --workspace   # Build everything
cargo test --workspace    # Run all 690+ tests
cargo clippy --workspace  # Check for common issues
```

## License

Dual-licensed under [Apache 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT).
