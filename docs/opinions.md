# Opinionated Defaults

Cabalist is strongly opinionated about how `.cabal` files should be structured. Every opinion is documented here with rationale, and every opinion is overridable via `cabalist.toml`.

## `cabal-version: 3.0`

New projects default to `cabal-version: 3.0`. This unlocks common stanzas (`common`, `import`) which are essential for maintainable `.cabal` files. We do not support `cabal-version < 2.2` for new projects.

## Default Language

- `GHC2021` if the detected GHC is >= 9.2
- `Haskell2010` otherwise

GHC2021 bundles a curated set of extensions that were previously opt-in, reducing boilerplate.

## GHC Options

```
ghc-options: -Wall -Wcompat -Widentities -Wincomplete-record-updates
             -Wincomplete-uni-patterns -Wmissing-deriving-strategies
             -Wredundant-constraints -Wunused-packages
```

**Rationale:** This catches most common mistakes without being so noisy that people disable warnings entirely. Each flag is documented in the GHC warning database.

## Default Extensions

```
default-extensions:
    OverloadedStrings
    DerivingStrategies
    DeriveGeneric
    DeriveAnyClass
    GeneralizedNewtypeDeriving
    LambdaCase
    TypeApplications
    ScopedTypeVariables
```

**Rationale:** These are widely considered safe defaults that reduce boilerplate without changing semantics in surprising ways.

**Notably absent:**
- `StrictData` — too opinionated for a default (changes evaluation semantics)
- `TemplateHaskell` — increases compile times, makes cross-compilation harder
- `UndecidableInstances` — type-checker footgun

## Directory Layout

```
project-name/
├── project-name.cabal
├── cabal.project
├── src/        # library source (hs-source-dirs: src)
├── app/        # executable source (hs-source-dirs: app)
├── test/       # test source (hs-source-dirs: test)
├── bench/      # benchmark source (hs-source-dirs: bench)
├── CHANGELOG.md
└── LICENSE
```

## License

Default: `MIT`. Shown as an option during `cabalist init`.

## Lints

All lints are individually disableable via `cabalist.toml`. Each lint has a unique string ID.

| Lint ID | Default Severity | What it checks |
|---------|-----------------|----------------|
| `missing-upper-bound` | Warning | Dependency has no upper version bound (violates PVP) |
| `missing-lower-bound` | Warning | Dependency has no lower version bound |
| `wide-any-version` | Warning | Dependency uses `>=0` or no constraint at all |
| `missing-synopsis` | Info | Package has no `synopsis` field |
| `missing-description` | Info | Package has no `description` field |
| `missing-source-repo` | Info | No `source-repository` section |
| `missing-bug-reports` | Info | No `bug-reports` field |
| `no-common-stanza` | Info | Multiple components share 5+ fields — suggest extracting a common stanza |
| `ghc-options-werror` | Warning | `-Werror` in non-conditional block (breaks downstream builds) |
| `missing-default-language` | Warning | No `default-language` in a component |
| `exposed-no-modules` | Error | Library with empty or missing `exposed-modules` |
| `string-gaps` | Info | Source directories or module names that don't match the filesystem |
| `cabal-version-low` | Info | `cabal-version < 3.0` — suggest upgrading to unlock features |
| `duplicate-dep` | Warning | Same package in `build-depends` more than once |
| `unused-flag` | Warning | A `flag` section exists but is never referenced in conditions |
| `stale-tested-with` | Info | `tested-with` lists a GHC version more than 2 major releases old |

## Recommended Packages

Cabalist maintains a curated database of recommended packages for common tasks (JSON, HTTP, testing, etc.). When searching for dependencies in the TUI, recommended packages are shown with a badge and an explanation of why they're recommended.

See `data/recommended-deps.toml` for the full database.
