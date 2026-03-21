# Configuration Reference

Cabalist is configured via `cabalist.toml`. This file controls linting rules, formatting behavior, and default values for new projects.

## Config File Location

Configuration is loaded from (in order of precedence):

1. **Project root** — `cabalist.toml` in the same directory as your `.cabal` file (highest priority)
2. **User config** — `~/.config/cabalist/config.toml` (or `$XDG_CONFIG_HOME/cabalist/config.toml`)
3. **Built-in defaults** — used if no config file is found

Project settings override user settings, which override built-in defaults.

## Complete Example

```toml
[defaults]
cabal-version = "3.0"
default-language = "GHC2021"
license = "MIT"

[defaults.ghc-options]
options = [
  "-Wall",
  "-Wcompat",
  "-Widentities",
  "-Wincomplete-record-updates",
  "-Wincomplete-uni-patterns",
  "-Wmissing-deriving-strategies",
  "-Wredundant-constraints",
  "-Wunused-packages",
]

[defaults.extensions]
extensions = [
  "OverloadedStrings",
  "DerivingStrategies",
  "DeriveGeneric",
  "DeriveAnyClass",
  "GeneralizedNewtypeDeriving",
  "LambdaCase",
  "TypeApplications",
  "ScopedTypeVariables",
]

[lints]
disable = ["stale-tested-with"]
error = ["missing-upper-bound", "wide-any-version"]

[formatting]
sort-dependencies = true
sort-modules = true
indent = 2
```

## Sections

### `[defaults]`

Controls default values used when creating new projects (`cabalist init` / init wizard).

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `cabal-version` | string | `"3.0"` | Default `cabal-version` for new projects |
| `default-language` | string | `"GHC2021"` | Default language standard. Auto-detected from GHC if not set (`GHC2021` for GHC >= 9.2, `Haskell2010` otherwise) |
| `license` | string | `"MIT"` | Default SPDX license identifier |

### `[defaults.ghc-options]`

Override the default GHC warning flags included in new projects and common stanza templates.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `options` | list of strings | See below | GHC command-line flags. **Replaces** the built-in set entirely (not merged) |

**Built-in default**:
```toml
options = [
  "-Wall",
  "-Wcompat",
  "-Widentities",
  "-Wincomplete-record-updates",
  "-Wincomplete-uni-patterns",
  "-Wmissing-deriving-strategies",
  "-Wredundant-constraints",
  "-Wunused-packages",
]
```

### `[defaults.extensions]`

Override the default language extensions included in new projects.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `extensions` | list of strings | See below | Extensions to enable. **Replaces** the built-in set entirely |

**Built-in default**:
```toml
extensions = [
  "OverloadedStrings",
  "DerivingStrategies",
  "DeriveGeneric",
  "DeriveAnyClass",
  "GeneralizedNewtypeDeriving",
  "LambdaCase",
  "TypeApplications",
  "ScopedTypeVariables",
]
```

### `[lints]`

Control which lints run and their severity.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `disable` | list of strings | `[]` | Lint IDs to disable entirely. Disabled lints produce no output |
| `error` | list of strings | `[]` | Lint IDs to promote to error severity. Useful with `cabalist-cli check --strict` |

**Available lint IDs** (see [Opinions & Lints](opinions.md) for full details):

```
missing-upper-bound      missing-lower-bound      wide-any-version
missing-synopsis         missing-description      missing-source-repo
missing-bug-reports      no-common-stanza         ghc-options-werror
missing-default-language exposed-no-modules       cabal-version-low
duplicate-dep            unused-flag              stale-tested-with
string-gaps
```

### `[formatting]`

Control formatting behavior for `cabalist-cli fmt` and `Ctrl+F` in the TUI.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `sort-dependencies` | bool | `true` | Sort `build-depends` entries alphabetically |
| `sort-modules` | bool | `true` | Sort `exposed-modules` and `other-modules` alphabetically |
| `indent` | integer | `2` | Indentation width in spaces |

## Environment

These environment variables affect cabalist behavior:

| Variable | Effect |
|----------|--------|
| `RUST_LOG` | Log verbosity for `cabalist-lsp` (`debug`, `info`, `warn`, `error`) |

## Auto-Detection

Cabalist auto-detects several settings at startup:

- **GHC version**: Runs `ghc --numeric-version` to determine `default-language` (GHC2021 vs Haskell2010)
- **Git author**: Runs `git config user.name` and `git config user.email` to pre-fill author/maintainer during `init`
- **Hackage cache**: Stored at `~/.cache/cabalist/` (or `$XDG_CACHE_HOME/cabalist/`)

## Recipes

### Minimal Linting (Library Author)

Only check version bounds, ignore style suggestions:

```toml
[lints]
disable = [
  "missing-synopsis",
  "missing-description",
  "missing-source-repo",
  "missing-bug-reports",
  "no-common-stanza",
  "stale-tested-with",
  "cabal-version-low",
]
```

### Strict CI (Open Source Project)

Promote all important checks to errors:

```toml
[lints]
error = [
  "missing-upper-bound",
  "missing-lower-bound",
  "wide-any-version",
  "duplicate-dep",
  "ghc-options-werror",
  "missing-default-language",
  "exposed-no-modules",
]
```

### No Sorting

If you prefer manual ordering of dependencies and modules:

```toml
[formatting]
sort-dependencies = false
sort-modules = false
```

### Custom Extension Set

If your team uses a different set of default extensions:

```toml
[defaults.extensions]
extensions = [
  "OverloadedStrings",
  "DerivingStrategies",
  "DataKinds",
  "TypeFamilies",
  "GADTs",
]
```
