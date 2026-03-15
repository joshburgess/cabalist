# Cabal Spec Compliance

This document tracks how the cabalist-parser handles every known syntax feature of the `.cabal` file format, based on the official Cabal specification and real-world testing against 100 packages from the Haskell ecosystem.

## Test Corpus

100 real-world `.cabal` files from popular Haskell packages, all passing byte-identical round-trip (`parse -> render == original`). Includes: pandoc (930 lines), lens (486 lines), Cabal itself, aeson, servant, warp, vector, text, bytestring, QuickCheck, and 90 others. Total: ~15,000 lines of real `.cabal` source.

## Syntax Features

### Layout

| Feature | Status | Notes |
|---------|--------|-------|
| Indentation-based layout | Supported | Primary layout mode |
| Brace-based layout on sections (`library { ... }`) | Supported | Per spec grammar `SectionLayoutOrBraces` |
| Brace-based layout on `if` (`if cond { ... }`) | Supported | |
| Brace-based layout on `else` (`else { ... }`) | Supported | Found in gauge.cabal |
| Braced freeform text blocks (`Description: { ... }`) | Supported | Found in cassava.cabal |
| Mixed brace/indentation in same file | Supported | gauge.cabal mixes both |
| CRLF line endings | Supported | Found in bifunctors.cabal |
| Tab indentation (expands to multiples of 8) | Supported | |
| Multi-line field values via indentation continuation | Supported | |
| Dot blank lines in descriptions (`.` on indented line) | Supported | Found in aeson, async, base-compat |

### Section Keywords (all 9)

| Keyword | Status | Notes |
|---------|--------|-------|
| `library` | Supported | Named and unnamed |
| `executable` | Supported | |
| `test-suite` | Supported | |
| `benchmark` | Supported | |
| `flag` | Supported | |
| `source-repository` | Supported | |
| `common` | Supported | With `import:` directive |
| `custom-setup` | Supported | Found in wreq.cabal |
| `foreign-library` | Supported | Added preemptively from spec |

### Conditional Syntax

| Feature | Status | Notes |
|---------|--------|-------|
| `if` / `else` blocks | Supported | |
| `elif` keyword | Supported | Not in official spec but harmlessly handled |
| `flag(name)` condition | Supported | |
| `os(name)` condition | Supported | |
| `arch(name)` condition | Supported | |
| `impl(compiler version-range)` condition | Supported | e.g., `impl(ghc >= 9.0)` |
| `impl(compiler)` without version | Supported | e.g., `impl(ghc)` |
| Boolean literals `true` / `false` | Supported | Case-insensitive (`True`, `FALSE`, etc.) |
| `!` (negation) | Supported | |
| `&&` (and) | Supported | |
| `||` (or) | Supported | |
| Parenthesized sub-expressions | Supported | e.g., `(flag(a) || flag(b)) && os(linux)` |
| Nested conditionals | Supported | |

### Version Ranges

| Feature | Status | Notes |
|---------|--------|-------|
| `==` (exact) | Supported | |
| `>=` (greater or equal) | Supported | |
| `>` (greater) | Supported | |
| `<=` (less or equal) | Supported | |
| `<` (less) | Supported | |
| `^>=` (PVP major bound) | Supported | `^>=2.2` means `>=2.2 && <2.3` |
| `&&` (intersection) | Supported | |
| `||` (union) | Supported | |
| Parenthesized ranges | Supported | |
| Wildcard `==1.2.*` | Supported | Desugars to `>=1.2 && <1.3` |
| `-any` keyword | Supported | Found in filepath.cabal, vector.cabal |
| `-none` keyword | Supported | |
| Set notation `^>= { 1.0, 2.0 }` | Supported | Cabal 3.0+ feature |
| Set notation `== { 1.0, 2.0 }` | Supported | Cabal 3.0+ feature |

### Field Handling

| Feature | Status | Notes |
|---------|--------|-------|
| Case-insensitive field names | Supported | `Build-Depends` == `build-depends` |
| Hyphen/underscore equivalence | Supported | `build-depends` == `build_depends` |
| `x-` custom/extension fields | Supported | Preserved as regular fields |
| `import:` directive | Supported | Parsed as distinct CST node kind |
| Multi-line list fields (all 4 styles) | Supported | Single-line, leading comma, trailing comma, no comma |
| Deprecated `cabal-version: >= x.y` | Supported | |
| Fields with no value (`build-depends:` alone) | Supported | |

### Fields Preserved as Raw Values (no special parsing needed)

These fields have complex value syntax but our parser correctly preserves them verbatim in the CST. The AST stores them in `other_fields`:

| Field | Notes |
|-------|-------|
| `mixins` | Backpack mixin syntax |
| `reexported-modules` | `orig-pkg:Name as NewName` syntax |
| `signatures` | Backpack module signatures |
| `virtual-modules` | Modules without source files |
| `autogen-modules` | Auto-generated modules |
| `pkgconfig-depends` | pkg-config dependency syntax |
| `frameworks` | macOS frameworks |
| `visibility` | `public` / `private` for internal libraries |
| Library qualifiers in deps | `pkg:lib` syntax in build-depends |

### Section Argument Types

Per the Cabal spec, section arguments can be:

| Type | Status | Notes |
|------|--------|-------|
| `SecArgName` (identifiers/numbers) | Supported | e.g., `executable my-exe` |
| `SecArgStr` (quoted strings) | Preserved | Extremely rare in practice |
| `SecArgOther` (operators) | Preserved | Extremely rare in practice |

## Issues Found via Real-World Testing

These issues were discovered by testing against real Haskell packages and fixed:

1. **Missing `custom-setup` keyword** — Found via wreq.cabal. Added to section keywords.
2. **Missing `foreign-library` keyword** — Added preemptively from spec review.
3. **Braced freeform text blocks** — Found via cassava.cabal (`Description: { ... }`).
4. **Braced conditional blocks** — Found via gauge.cabal (`else { ... }`).
5. **CRLF trailing content duplication** — Found via bifunctors.cabal. Root cause was a section nesting bug at indent 0 plus duplicated trailing trivia consumption.
6. **Boolean literals in conditions** — Found via spec review.
7. **Wildcard version ranges** — Found in attoparsec.cabal, aeson-pretty.cabal.
8. **`-any`/`-none` version keywords** — Found in filepath.cabal, vector.cabal.
9. **Set notation for version ranges** — Found via spec review (no usage in corpus, but spec supports it).

## Test Corpus Packages

The 100 packages tested (alphabetical):

adjunctions, aeson, aeson-pretty, ansi-terminal, async, attoparsec,
base-compat, base-orphans, bifunctors, blaze-html, bytestring,
Cabal, Cabal-syntax, cassava, clay, colour, comonad, conduit,
containers, contravariant, criterion, crypton,
data-default, deepseq, directory, distributive, dlist,
esqueleto, exceptions,
filepath, free,
gauge, generic-lens,
hashable, hedgehog, hlint, hspec, hspec-core, http-client,
http-conduit, http-types,
kan-extensions,
lens, lifted-base, lucid,
megaparsec, memory, microlens, monad-control, mono-traversable, mtl,
network,
optics, optparse-applicative,
pandoc, parsec, persistent, pretty, prettyprinter, primitive,
process, profunctors,
QuickCheck,
random, req, resourcet, rio,
safe, safe-exceptions, scientific, scotty, semialign,
semigroupoids, servant, shake, stm,
tagged, tasty, template-haskell, text, these, tls,
transformers, turtle, typed-process,
unix, unliftio, unliftio-core, unordered-containers, uuid-types,
vector, void,
wai, warp, witherable, wreq,
yaml, yesod
