# Getting Started

This guide walks you through common cabalist workflows with step-by-step instructions.

## Scenario 1: Starting a New Haskell Project

### Using the TUI

```sh
mkdir my-project && cd my-project
cabalist
```

The init wizard starts automatically since no `.cabal` file exists:

1. **Name**: Type your project name (defaults to `my-project`). Press `Enter`.
2. **Template**: Press `Tab` to cycle through options: Library, Application, Library + Application, Full. Press `Enter`.
3. **License**: Type a license identifier (e.g., `MIT`, `BSD-3-Clause`). Press `Enter`.
4. **Author**: Pre-filled from `git config`. Edit if needed. Press `Enter`.
5. **Synopsis**: Type a one-line description. Press `Enter`.
6. **Confirm**: Review your choices. Press `Enter` to create the project.

Cabalist creates the `.cabal` file with a `common` stanza, proper GHC warnings, `cabal-version: 3.0`, and the appropriate directory structure.

### Using the CLI

```sh
mkdir my-project && cd my-project
cabalist-cli init --name my-project --type lib-and-exe --license MIT
```

This creates:
```
my-project/
├── my-project.cabal
├── src/MyProject.hs
├── app/Main.hs
```

For a full project with tests and benchmarks:

```sh
cabalist-cli init --type full
```

---

## Scenario 2: Adding Dependencies to an Existing Project

### Using the TUI

1. Launch `cabalist` in your project directory.
2. Press `d` to go to the Dependencies view.
3. Press `a` to open the Hackage search popup.
4. Type `aeson` (or any package name). Results appear ranked by relevance.
5. Use arrow keys to select the right package, then press `Enter`.
6. Cabalist adds the dependency with correct PVP bounds (e.g., `aeson ^>=2.2`).
7. Press `Ctrl+S` to save.

### Using the CLI

```sh
cabalist-cli add aeson                        # Auto PVP bounds
cabalist-cli add text --version "^>=2.0"      # Explicit version
cabalist-cli add warp --component exe:server  # Add to a specific executable
```

To remove a dependency:

```sh
cabalist-cli remove old-package
```

---

## Scenario 3: Checking for Issues Before Publishing

### Quick Check

```sh
cabalist-cli check
```

This runs all 16 lints and reports issues:

```
my-lib.cabal:1:1: warning: Dependency 'text' has no upper version bound [missing-upper-bound]
  suggestion: Use '^>=' for PVP-compliant major bounds
my-lib.cabal:1:1: info: Package is missing a 'description' field [missing-description]
  suggestion: Add a description for your package's Hackage page
```

### Strict Mode (for CI)

```sh
cabalist-cli check --strict
```

With `--strict`, warnings are treated as errors. Exit code 1 means there are issues to fix.

### Fix Issues Interactively

Launch the TUI (`cabalist`) and check the Dashboard's health section. It shows all lints with their severity. Use the Metadata view (`m`) to add missing fields, and the Dependencies view (`d`) to fix version bounds.

---

## Scenario 4: Setting Up CI/CD

### GitHub Actions

Add to `.github/workflows/cabal-lint.yml`:

```yaml
name: Cabal Lint

on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install cabalist-cli
        run: |
          curl -fsSL https://github.com/joshburgess/cabalist/releases/latest/download/cabalist-cli-x86_64-unknown-linux-gnu -o cabalist-cli
          chmod +x cabalist-cli
          sudo mv cabalist-cli /usr/local/bin/

      - name: Check .cabal file
        run: cabalist-cli check --strict

      - name: Verify formatting
        run: cabalist-cli fmt --check

      - name: Check for unlisted modules
        run: cabalist-cli modules --scan
```

### Strict Configuration for CI

Create `cabalist.toml` in your project root:

```toml
[lints]
error = [
  "missing-upper-bound",
  "missing-lower-bound",
  "wide-any-version",
  "duplicate-dep",
  "exposed-no-modules",
]

[formatting]
sort-dependencies = true
sort-modules = true
```

---

## Scenario 5: Managing GHC Extensions

### Using the TUI

1. Press `e` from the dashboard to open the Extensions view.
2. Enabled extensions appear first with `[x]`. Available extensions follow with `[ ]`.
3. Use `j`/`k` to navigate. Press `Space` to toggle an extension on/off.
4. Press `i` on any extension to see its description, GHC version, category, and safety information.
5. Use `/` to filter by name (e.g., type `Deriv` to find all deriving extensions).
6. Press `Ctrl+S` to save.

### Using the CLI

```sh
cabalist-cli extensions                            # List enabled extensions
cabalist-cli extensions --toggle DerivingStrategies # Enable/disable
cabalist-cli extensions --toggle LambdaCase --component exe:my-app
```

---

## Scenario 6: Setting Up Editor Integration

### VS Code

1. Install `cabalist-lsp`:
   ```sh
   cargo install --path crates/cabalist-lsp
   ```

2. Build the VS Code extension:
   ```sh
   cd editors/vscode && npm install && npm run compile
   ```

3. In VS Code, run "Developer: Install Extension from Location..." and select the `editors/vscode` directory.

4. Open a `.cabal` file. You should see:
   - Syntax highlighting
   - Diagnostics (yellow/red squiggles for lint issues)
   - Completions (type in `build-depends:` to see package suggestions)
   - Hover documentation (hover over field names, extensions, packages)
   - Code actions (click the lightbulb to fix issues)

### Neovim

See [Editor Setup](editor-setup.md) for nvim-lspconfig or native vim.lsp.start configuration.

### Emacs

See [Editor Setup](editor-setup.md) for eglot or lsp-mode configuration.

---

## Scenario 7: Migrating an Existing Project

If you have an existing project with a `.cabal` file that hasn't been using cabalist:

### Step 1: Run a Health Check

```sh
cd my-existing-project
cabalist-cli check
```

This shows all the issues cabalist would flag. Don't worry about fixing everything at once.

### Step 2: Format the File

```sh
cabalist-cli fmt
```

This normalizes whitespace and optionally sorts dependencies/modules. Use `--check` first to preview:

```sh
cabalist-cli fmt --check   # Shows if changes are needed without applying
cabalist-cli fmt           # Apply formatting
```

### Step 3: Fix Version Bounds

The most impactful fix is adding proper PVP bounds. In the TUI:

1. Press `d` for Dependencies.
2. Look for `! no upper bound` indicators.
3. For each one, you can remove and re-add with proper bounds, or edit the `.cabal` file directly.

Or use the LSP code actions: open the file in your editor, and click the lightbulb on each `missing-upper-bound` warning.

### Step 4: Add Missing Metadata

```sh
cabalist-cli set synopsis "My library for doing X"
cabalist-cli set description "Please see the README"
cabalist-cli set bug-reports "https://github.com/user/repo/issues"
```

### Step 5: Add a Common Stanza

If you have multiple components with duplicated settings, extract them:

```cabal
common warnings
  default-language: GHC2021
  ghc-options: -Wall -Wcompat -Widentities
  default-extensions: OverloadedStrings

library
  import: warnings
  ...

test-suite tests
  import: warnings
  ...
```

The `no-common-stanza` lint will tell you when this is beneficial (5+ shared fields).

### Step 6: Scan for Module Issues

```sh
cabalist-cli modules --scan
```

This finds `.hs` files on disk that aren't listed in the `.cabal` file, and vice versa.

---

## Scenario 8: Keeping Dependencies Up to Date

### Check for Outdated Packages

First, ensure you have a cached Hackage index:

```sh
cabalist-cli update-index
```

Then check for outdated dependencies:

```sh
cabalist-cli deps --outdated
```

Output:
```
library
  text                           ^>=1.2                    -> latest: 2.1
  aeson                          ^>=2.0                    -> latest: 2.2.3.0
```

### In the TUI

Press `Ctrl+U` to update the index, then go to Dependencies (`d`). Outdated packages show a red `-> X.Y.Z` indicator. The Dashboard also shows an outdated count in the health section.

### Visualize the Dependency Tree

```sh
cabalist-cli deps --tree
```

In the TUI, press `v` in the Dependencies view to toggle tree mode.

---

## Scenario 9: Working with cabal.project

If your project uses a multi-package `cabal.project` file:

### In the TUI

1. Press `p` from the dashboard to open the Project view.
2. The overview shows: packages, compiler, index-state, source-repository-packages.
3. The details section shows: constraints, allow-newer, allow-older, package stanzas.
4. Use `Enter` to edit the `with-compiler` or `index-state` fields inline.

### Common cabal.project Fields

```
packages: ./*.cabal           -- Which packages to include
with-compiler: ghc-9.8.2      -- Pin a specific GHC version
index-state: 2026-01-01T00:00:00Z  -- Pin Hackage state for reproducibility
allow-newer: all:base          -- Relax upper bounds
constraints: aeson ^>=2.2      -- Global version constraints
```

---

## Scenario 10: Team Workflow

### Shared Configuration

Commit `cabalist.toml` to your repository so all team members and CI use the same lint rules:

```toml
# cabalist.toml — shared team configuration
[lints]
error = ["missing-upper-bound", "duplicate-dep"]
disable = ["stale-tested-with"]

[formatting]
sort-dependencies = true
sort-modules = true

[defaults]
default-language = "GHC2021"
license = "BSD-3-Clause"
```

### Developer Workflow

1. Edit code normally.
2. Before committing, run `cabalist-cli check` and `cabalist-cli fmt`.
3. Or use the TUI for interactive editing with real-time lint feedback.
4. Or use the LSP in your editor for inline diagnostics and quick fixes.

### CI Integration

Add `cabalist-cli check --strict && cabalist-cli fmt --check` to your CI pipeline. This catches version bound issues, missing metadata, and formatting drift.

---

## Common Troubleshooting

### "No .cabal file found"

Cabalist auto-detects `.cabal` files in the current directory. Use `--file` to specify one explicitly:

```sh
cabalist --file packages/my-lib/my-lib.cabal
cabalist-cli check --file my-lib.cabal
```

### "Hackage index not found"

Run `cabalist-cli update-index` or press `Ctrl+U` in the TUI. The index is cached at `~/.cache/cabalist/index.json`.

### "Multiple .cabal files found"

In a multi-package repository, use `--file` to specify which `.cabal` file to work with.

### Formatting Changes Unexpected Fields

Cabalist's parser preserves your formatting byte-for-byte unless you explicitly format (`fmt` or `Ctrl+F`). If a save is changing things you didn't edit, check that you're using `Ctrl+S` (save) not `Ctrl+F` (format).

### LSP Not Starting

Check that `cabalist-lsp` is on your PATH:

```sh
which cabalist-lsp
cabalist-lsp --help
```

Check the server logs:

```sh
RUST_LOG=debug cabalist-lsp --stdio 2>lsp.log
```
