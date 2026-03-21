# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Parser**: CST/AST parser for `.cabal` files with byte-identical round-trip fidelity
- **Parser**: Error recovery — continues parsing after syntax errors, collecting diagnostics
- **Parser**: Property-based round-trip testing with proptest
- **Opinions**: 16 opinionated lints (version bounds, documentation, structure, build, flags, filesystem)
- **Opinions**: Configurable via `cabalist.toml` with hierarchical overrides (project > user > defaults)
- **Opinions**: 4 project templates (library, application, lib-and-exe, full)
- **GHC**: 200+ GHC extensions with metadata (description, safety, since-version, category)
- **GHC**: GHC warnings database with group membership
- **Hackage**: Package index download, caching, and search
- **Hackage**: PVP-compliant version bound computation
- **Hackage**: Preferred-versions parsing for deprecation detection
- **Hackage**: Curated recommended packages database
- **TUI**: Interactive 6-view application (Dashboard, Dependencies, Extensions, Metadata, Build, Init)
- **TUI**: Live editing with undo/redo stack
- **TUI**: File watcher for external change detection
- **TUI**: 6-step init wizard for new Haskell projects
- **TUI**: Async build/test/clean with streaming output and error navigation
- **TUI**: Hackage package search integration in dependency view
- **TUI**: Inline metadata editing
- **CLI**: Non-interactive commands: init, add, remove, check, fmt, deps, modules, info
- **CLI**: JSON output format for scripting/CI
- **LSP**: Language server with diagnostics, completions, hover, and code actions
- **LSP**: Hackage package name completions with PVP bounds
- **LSP**: GHC extension and warning flag completions with documentation
- **LSP**: Context-aware field name completions
- **LSP**: Smart code actions that derive defaults from package metadata
- **VS Code**: Extension with TextMate syntax highlighting, snippets, and LSP client
- **VS Code**: Status bar, restart command, output channel, cabal.project file support
- **Editors**: Neovim and Emacs LSP configuration guides
- **Project**: `cabal.project` file parser
- **Testing**: 621+ tests including real-world corpus of 100 `.cabal` files
- **CI**: Cross-platform GitHub Actions (Linux, macOS, Windows) with MSRV 1.75

## [0.1.0] - Unreleased

Initial release.
