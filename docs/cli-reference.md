# CLI Reference

`cabalist-cli` is a non-interactive command-line tool for managing Haskell `.cabal` files. It is designed for scripting, CI pipelines, and quick terminal edits.

## Global Flags

```
--file <PATH>      Path to the .cabal file (auto-detected if omitted)
--format <FORMAT>  Output format: text (default) or json
--version          Print version
--help             Print help
```

## Commands

### `init` — Create a New Project

```sh
cabalist-cli init
cabalist-cli init --name my-library --type library --license BSD-3-Clause
cabalist-cli init --type full --author "Jane Doe" --minimal
```

| Flag | Default | Description |
|------|---------|-------------|
| `--name <NAME>` | Directory name | Project name |
| `--type <TYPE>` | `lib-and-exe` | `library`, `application`, `lib-and-exe`, or `full` |
| `--license <LICENSE>` | `MIT` | SPDX license identifier |
| `--author <AUTHOR>` | From `git config` | Author name |
| `--minimal` | false | Only create `.cabal` file, no directory scaffolding |

**Templates**:
- `library` — Library only (src/)
- `application` — Executable only (app/)
- `lib-and-exe` — Library + executable (src/ + app/)
- `full` — Library + executable + test-suite + benchmark (src/ + app/ + test/ + bench/)

The generated `.cabal` file includes a `common` stanza with recommended GHC warnings and the appropriate `default-language` for your GHC version.

---

### `add` — Add a Dependency

```sh
cabalist-cli add aeson                          # Add with auto PVP bounds
cabalist-cli add text --version "^>=2.0"        # Add with explicit constraint
cabalist-cli add warp --component exe:my-server # Add to a specific component
```

| Flag | Default | Description |
|------|---------|-------------|
| `--version <SPEC>` | None (package name only) | Version constraint (e.g., `^>=2.0`, `>=1.0 && <2.0`) |
| `--component <SPEC>` | `library` | Target component: `library`, `exe:<name>`, `test:<name>`, `bench:<name>` |

Dependencies are inserted in alphabetical order. Adding a duplicate fails with an error.

---

### `remove` — Remove a Dependency

```sh
cabalist-cli remove old-package
cabalist-cli remove warp --component exe:my-server
```

| Flag | Default | Description |
|------|---------|-------------|
| `--component <SPEC>` | `library` | Target component |

---

### `extensions` — List or Toggle GHC Extensions

```sh
cabalist-cli extensions                            # List enabled extensions
cabalist-cli extensions --toggle DerivingStrategies # Toggle an extension
cabalist-cli extensions --toggle LambdaCase --component exe:my-app
```

| Flag | Default | Description |
|------|---------|-------------|
| `--toggle <NAME>` | None | Extension to toggle on/off |
| `--component <SPEC>` | `library` | Target component |

Without `--toggle`, lists currently enabled extensions with descriptions. Shows the count of available extensions not yet enabled.

---

### `set` — Set a Metadata Field

```sh
cabalist-cli set synopsis "Fast JSON parsing for Haskell"
cabalist-cli set version 1.0.0.0
cabalist-cli set license BSD-3-Clause
cabalist-cli set homepage "https://github.com/user/repo"
```

**Settable fields**: `name`, `version`, `cabal-version`, `synopsis`, `description`, `license`, `license-file`, `author`, `maintainer`, `homepage`, `bug-reports`, `category`, `build-type`, `tested-with`

If the field exists, its value is replaced. If not, a new field is added at the root level.

---

### `check` — Run Lints

```sh
cabalist-cli check               # Show warnings and suggestions
cabalist-cli check --strict      # Treat warnings as errors (exit code 1)
cabalist-cli check --format json # JSON output for tooling
```

| Flag | Default | Description |
|------|---------|-------------|
| `--strict` | false | Promote warnings to errors |

Runs all 16 opinionated lints plus parser validation. Output follows GCC diagnostic format: `file:line:col: severity: message [lint-id]`.

**Exit codes**: 0 = clean, 1 = warnings (or errors with `--strict`), 2 = errors.

See [Opinions & Lints](opinions.md) for the full lint reference.

---

### `fmt` — Format

```sh
cabalist-cli fmt          # Format in place
cabalist-cli fmt --check  # Check without modifying (exit 1 if unformatted)
```

| Flag | Default | Description |
|------|---------|-------------|
| `--check` | false | Validate formatting without writing |

Formatting performs a round-trip through the parser (normalizing whitespace) and optionally sorts `build-depends` and `exposed-modules` alphabetically. Sorting is controlled by `cabalist.toml`:

```toml
[formatting]
sort-dependencies = true
sort-modules = true
```

---

### `deps` — Dependency Information

```sh
cabalist-cli deps               # Flat list with PVP status
cabalist-cli deps --tree        # ASCII tree view
cabalist-cli deps --outdated    # Show packages with newer Hackage versions
```

| Flag | Default | Description |
|------|---------|-------------|
| `--tree` | false | Show as dependency tree |
| `--outdated` | false | Compare against Hackage latest |

**PVP status**: Each dependency shows `ok` (both bounds), `no upper bound`, `no lower bound`, or `no bounds`.

**Outdated**: Requires a cached Hackage index. Run `cabalist-cli update-index` first if the cache is missing.

---

### `modules` — Module Management

```sh
cabalist-cli modules               # List exposed and other modules
cabalist-cli modules --scan        # Find unlisted .hs files
cabalist-cli modules --component exe:my-app
```

| Flag | Default | Description |
|------|---------|-------------|
| `--scan` | false | Scan source directories for `.hs` files not in `.cabal` |
| `--component <SPEC>` | `library` | Target component |

The scan recursively searches `hs-source-dirs` for `.hs` files and reports any that aren't listed in `exposed-modules` or `other-modules`.

---

### `info` — Project Summary

```sh
cabalist-cli info
cabalist-cli info --format json
```

Shows: package name, version, license, author, cabal-version, synopsis, components (with module/dep counts), common stanzas, flags, and health summary.

---

### `build` / `test` / `clean` — Build Integration

```sh
cabalist-cli build    # Run cabal build with streaming output
cabalist-cli test     # Run cabal test with streaming output
cabalist-cli clean    # Run cabal clean
```

Output is streamed line-by-line. Exit code reflects the build result.

---

### `update-index` — Refresh Hackage Index

```sh
cabalist-cli update-index
```

Downloads the Hackage package index (~150MB) and caches it at `~/.cache/cabalist/index.json`. Required for `deps --outdated`, Hackage search in the TUI, and LSP inlay hints.

---

## CI Recipes

### Lint on Every PR

```yaml
- name: Lint .cabal file
  run: cabalist-cli check --strict
```

### Check Formatting

```yaml
- name: Check .cabal formatting
  run: cabalist-cli fmt --check
```

### Find Unlisted Modules

```yaml
- name: Check for unlisted modules
  run: cabalist-cli modules --scan
```

### Full CI Check

```yaml
- name: Cabalist checks
  run: |
    cabalist-cli check --strict
    cabalist-cli fmt --check
    cabalist-cli modules --scan
```

### JSON Output for Tooling

```yaml
- name: Check with JSON output
  run: cabalist-cli check --format json > lint-results.json
```

## Component Specifiers

Several commands accept a `--component` flag. The format is:

| Specifier | Meaning |
|-----------|---------|
| `library` | The default (unnamed) library |
| `exe:<name>` | An executable component |
| `test:<name>` | A test-suite component |
| `bench:<name>` | A benchmark component |

Example: `cabalist-cli add warp --component exe:my-server`
