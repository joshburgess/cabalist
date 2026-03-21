# Opinions & Lints

Cabalist ships 16 opinionated lints that check your `.cabal` file for common issues, missing metadata, and PVP compliance. Every lint is individually configurable — you can disable it, change its severity, or promote it to an error.

## Lint Reference

### Version Bounds

#### `missing-upper-bound` (Warning)

**What it checks**: A dependency has a lower bound but no upper bound (e.g., `>=4.17` instead of `^>=4.17`).

**Why it matters**: Without an upper bound, your package may silently break when a dependency releases a new major version. The [Package Versioning Policy (PVP)](https://pvp.haskell.org/) requires upper bounds.

**Quick fix (LSP)**: Replaces `>=X.Y` with `^>=X.Y`.

```
-- Bad:  build-depends: base >=4.17
-- Good: build-depends: base ^>=4.17
```

#### `missing-lower-bound` (Warning)

**What it checks**: A dependency has an upper bound but no lower bound (e.g., `<5`).

**Why it matters**: Without a lower bound, cabal may select a version too old to compile against.

**Quick fix (LSP)**: Converts to PVP `^>=` bounds when possible.

#### `wide-any-version` (Warning)

**What it checks**: A dependency has no version constraint at all, or uses `-any`.

**Why it matters**: Accepting any version is fragile. Even `base` changes its API across major GHC releases.

**Quick fix (LSP)**: Adds placeholder `^>=0.1` bounds.

```
-- Bad:  build-depends: text
-- Good: build-depends: text ^>=2.0
```

### Documentation

#### `missing-synopsis` (Info)

**What it checks**: The package has no `synopsis` field.

**Why it matters**: The synopsis appears in Hackage search results. Without it, your package is harder to discover.

**Quick fix (LSP)**: Inserts `synopsis:` using the package name as placeholder text.

#### `missing-description` (Info)

**What it checks**: The package has no `description` field.

**Why it matters**: The description appears on your Hackage package page.

**Quick fix (LSP)**: Inserts `description: Please see the README for <package-name>`.

#### `missing-source-repo` (Info)

**What it checks**: No `source-repository` section exists.

**Why it matters**: Users and contributors need to find your source code. Hackage displays the repository link prominently.

**Quick fix (LSP)**: Inserts a `source-repository head` section, deriving the URL from `homepage` if it's a GitHub/GitLab URL.

#### `missing-bug-reports` (Info)

**What it checks**: No `bug-reports` field exists.

**Why it matters**: Users need to know where to report bugs.

**Quick fix (LSP)**: Inserts `bug-reports:` derived from the `homepage` URL (appending `/issues` for GitHub/GitLab).

### Structure

#### `no-common-stanza` (Info)

**What it checks**: Multiple components (2+) share 5 or more identical field names, but no `common` stanza exists.

**Why it matters**: Duplicated settings across components are hard to maintain. A `common` stanza with `import:` keeps things DRY.

**Quick fix (LSP)**: Inserts a `common warnings` stanza template with recommended GHC options and default language.

```cabal
common warnings
  ghc-options: -Wall
  default-language: GHC2021

library
  import: warnings
```

#### `exposed-no-modules` (Error)

**What it checks**: A library section has no `exposed-modules` field, or the field is empty.

**Why it matters**: A library that exposes no modules is useless to consumers.

**Quick fix (LSP)**: Inserts `exposed-modules: MyModule`.

#### `cabal-version-low` (Info)

**What it checks**: `cabal-version` is below 3.0.

**Why it matters**: `cabal-version: 3.0` unlocks `common` stanzas and `import` directives, which are essential for maintainable `.cabal` files.

**Quick fix (LSP)**: Replaces the cabal-version value with `3.0`.

### Build

#### `ghc-options-werror` (Warning)

**What it checks**: `-Werror` appears in a component's top-level `ghc-options` (not inside a conditional).

**Why it matters**: `-Werror` breaks downstream builds when GHC introduces new warnings. It should only appear in CI-specific conditionals.

```cabal
-- Bad:
library
  ghc-options: -Wall -Werror

-- Good:
library
  ghc-options: -Wall
  if flag(ci)
    ghc-options: -Werror
```

#### `missing-default-language` (Warning)

**What it checks**: A component has no `default-language` field.

**Why it matters**: Without it, Cabal picks a default that may not match your expectations. Being explicit avoids surprises.

**Quick fix (LSP)**: Inserts `default-language: GHC2021`.

#### `duplicate-dep` (Warning)

**What it checks**: The same package appears more than once in a component's `build-depends`.

**Why it matters**: Duplicates are confusing and may cause build plan issues.

**Quick fix (LSP)**: Removes the duplicate line.

### Flags

#### `unused-flag` (Warning)

**What it checks**: A `flag` section is defined but never referenced in any `if flag(...)` conditional.

**Why it matters**: Unused flags add complexity without purpose. They confuse users who try to use them.

**Quick fix (LSP)**: Removes the entire flag section.

### Filesystem

#### `string-gaps` (Info)

**What it checks**: Source directories listed in `hs-source-dirs` that don't exist on disk, and modules listed in `exposed-modules` or `other-modules` that don't have a corresponding `.hs` file.

**Why it matters**: Mismatches between the `.cabal` file and the filesystem cause confusing build errors.

**Note**: This lint requires filesystem access and only runs when the project root is available.

#### `stale-tested-with` (Info)

**What it checks**: `tested-with` lists a GHC version more than 2 major releases behind the current series (9.12 as of 2026).

**Why it matters**: Stale `tested-with` entries mislead users about which GHC versions are actually supported.

**Quick fix (LSP)**: Removes the `tested-with` field entirely (you should re-add it with current versions).

## Configuration

All lints are configurable via `cabalist.toml`. See [Configuration](configuration.md) for the full reference.

### Disable a Lint

```toml
[lints]
disable = ["no-common-stanza", "stale-tested-with"]
```

### Promote to Error

```toml
[lints]
error = ["missing-upper-bound", "wide-any-version"]
```

### Example: Strict CI Configuration

```toml
[lints]
error = [
  "missing-upper-bound",
  "missing-lower-bound",
  "wide-any-version",
  "duplicate-dep",
  "ghc-options-werror",
  "exposed-no-modules",
]
disable = ["stale-tested-with"]
```

## Recommended Defaults

Cabalist's defaults are chosen for modern Haskell development:

**Default language**: `GHC2021` (or `Haskell2010` if GHC < 9.2)

**Default GHC warnings**:
```
-Wall -Wcompat -Widentities -Wincomplete-record-updates
-Wincomplete-uni-patterns -Wmissing-deriving-strategies
-Wredundant-constraints -Wunused-packages
```

**Default extensions**:
```
OverloadedStrings    DerivingStrategies    DeriveGeneric
DeriveAnyClass       GeneralizedNewtypeDeriving    LambdaCase
TypeApplications     ScopedTypeVariables
```

**Default cabal-version**: `3.0` (unlocks common stanzas)

## Recommended Packages

Cabalist maintains a curated database of recommended packages for 24 common tasks (JSON, HTTP, testing, web frameworks, databases, etc.). This powers the Hackage search completions and the TUI dependency search.

Each category specifies a recommended package, companion packages (commonly used together), and alternatives. For example:

- **JSON**: aeson (recommended); alternatives: json, jsonifier
- **Testing**: tasty (recommended); companions: tasty-hunit, tasty-quickcheck; alternatives: hspec, sydtest
- **Web Framework**: servant (recommended); companions: servant-server, warp; alternatives: scotty, yesod
- **Effect Systems**: mtl (recommended); companions: transformers; alternatives: effectful, polysemy, bluefin
