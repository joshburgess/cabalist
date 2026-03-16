# Configuration Reference

Cabalist is configured via `cabalist.toml`. Configuration is loaded from:

1. **Project root** — `cabalist.toml` in the same directory as your `.cabal` file
2. **User config** — `~/.config/cabalist/config.toml` (or `$XDG_CONFIG_HOME/cabalist/config.toml`)
3. **Built-in defaults** — if no config file is found

Project-level settings override user-level settings.

## Full Example

```toml
[defaults]
cabal-version = "3.0"
default-language = "GHC2021"
license = "MIT"

[defaults.ghc-options]
options = ["-Wall", "-Wcompat", "-Werror=incomplete-patterns"]

[defaults.extensions]
extensions = ["OverloadedStrings", "DerivingStrategies", "StrictData"]

[lints]
disable = ["missing-description"]
error = ["missing-upper-bound"]

[formatting]
sort-dependencies = true
sort-modules = true
indent = 2
```

## `[defaults]`

Controls default values used when creating new projects or components.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `cabal-version` | string | `"3.0"` | Default `cabal-version` for new projects |
| `default-language` | string | `"GHC2021"` | Default language (auto-detected from GHC if not set) |
| `license` | string | `"MIT"` | Default license for new projects |

### `[defaults.ghc-options]`

Override the default GHC warning flags. **Replaces** the built-in set entirely.

```toml
[defaults.ghc-options]
options = ["-Wall", "-Wcompat"]
```

Built-in defaults:
```
-Wall -Wcompat -Widentities -Wincomplete-record-updates
-Wincomplete-uni-patterns -Wmissing-deriving-strategies
-Wredundant-constraints -Wunused-packages
```

### `[defaults.extensions]`

Override the default language extensions. **Replaces** the built-in set entirely.

```toml
[defaults.extensions]
extensions = ["OverloadedStrings", "StrictData"]
```

Built-in defaults:
```
OverloadedStrings DerivingStrategies DeriveGeneric DeriveAnyClass
GeneralizedNewtypeDeriving LambdaCase TypeApplications ScopedTypeVariables
```

## `[lints]`

Control which lints are enabled and their severity.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `disable` | list of strings | `[]` | Lint IDs to disable entirely |
| `error` | list of strings | `[]` | Lint IDs to promote to error severity |

```toml
[lints]
disable = ["missing-description", "cabal-version-low"]
error = ["missing-upper-bound"]
```

See [opinions.md](opinions.md) for the full list of lint IDs.

## `[formatting]`

Controls how `cabalist-cli fmt` formats your `.cabal` file.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `sort-dependencies` | bool | `true` | Sort `build-depends` entries alphabetically |
| `sort-modules` | bool | `true` | Sort `exposed-modules` and `other-modules` alphabetically |
| `indent` | integer | `2` | Indentation width in spaces |

```toml
[formatting]
sort-dependencies = false
sort-modules = true
indent = 4
```

## Environment

Cabalist also respects the following environment:

- **GHC detection** — Cabalist runs `ghc --numeric-version` at startup to detect the installed GHC version. This affects the default language selection (GHC2021 vs Haskell2010) and extension availability filtering.
- **Git config** — `git config user.name` and `git config user.email` are used to auto-fill author/maintainer during `cabalist init`.
- **Hackage cache** — The package index is cached at `~/.cache/cabalist/` (or `$XDG_CACHE_HOME/cabalist/`).
