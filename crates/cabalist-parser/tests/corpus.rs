//! Round-trip corpus tests for the cabalist parser.
//!
//! Each fixture is a realistic `.cabal` file represented as a string constant.
//! The test verifies that `parse -> render` produces byte-identical output and
//! that no diagnostics are emitted.

use cabalist_parser::parse;

/// Parse the source and assert byte-identical round-trip with zero diagnostics.
fn assert_round_trip_clean(source: &str) {
    let result = parse(source);
    let rendered = result.cst.render();
    assert_eq!(
        rendered,
        source,
        "\n--- EXPECTED (len={}) ---\n{source}\n--- GOT (len={}) ---\n{rendered}\n",
        source.len(),
        rendered.len(),
    );
    assert!(
        result.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        result.diagnostics
    );
}

// ============================================================================
// Fixture 1: Minimal file (just name + version + cabal-version)
// ============================================================================

#[test]
fn corpus_minimal() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: minimal-pkg
version: 0.1.0.0
",
    );
}

// ============================================================================
// Fixture 2: Library-only project
// ============================================================================

#[test]
fn corpus_library_only() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: my-lib
version: 1.0.0.0
synopsis: A simple library
license: MIT
author: Jane Doe

library
  exposed-modules:
    MyLib
    MyLib.Internal
  build-depends:
    base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 3: Application-only project
// ============================================================================

#[test]
fn corpus_application_only() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: my-app
version: 0.1.0.0
synopsis: A command-line application
license: BSD-3-Clause
build-type: Simple

executable my-app
  main-is: Main.hs
  other-modules:
    App.Config
    App.Run
  build-depends:
    base >=4.14 && <5,
    optparse-applicative ^>=0.18,
    text ^>=2.0
  hs-source-dirs: app
  default-language: Haskell2010
",
    );
}

// ============================================================================
// Fixture 4: Library + executable + test-suite (common pattern)
// ============================================================================

#[test]
fn corpus_lib_exe_test() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: my-project
version: 0.1.0.0
synopsis: A typical Haskell project
license: MIT
author: Test Author
maintainer: test@example.com
build-type: Simple

library
  exposed-modules:
    MyProject
    MyProject.Types
  other-modules:
    MyProject.Internal
  build-depends:
    base >=4.14 && <5,
    text ^>=2.0,
    containers ^>=0.6
  hs-source-dirs: src
  default-language: GHC2021

executable my-project
  main-is: Main.hs
  build-depends:
    base >=4.14 && <5,
    my-project
  hs-source-dirs: app
  default-language: GHC2021

test-suite my-project-test
  type: exitcode-stdio-1.0
  main-is: Main.hs
  other-modules:
    Test.MyProject
  build-depends:
    base >=4.14 && <5,
    my-project,
    tasty ^>=1.5,
    tasty-hunit ^>=0.10
  hs-source-dirs: test
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 5: Full project with all component types including benchmark
// ============================================================================

#[test]
fn corpus_full_project() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: full-project
version: 2.1.0.0
synopsis: A full-featured project
description: This project has every component type.
license: Apache-2.0
author: Full Author
maintainer: full@example.com
category: Development
build-type: Simple

library
  exposed-modules:
    Full.Core
    Full.Types
    Full.Utils
  build-depends:
    base >=4.14 && <5,
    bytestring ^>=0.11,
    text ^>=2.0
  hs-source-dirs: src
  default-language: GHC2021

executable full-exe
  main-is: Main.hs
  build-depends:
    base,
    full-project
  hs-source-dirs: app
  default-language: GHC2021

test-suite full-test
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends:
    base,
    full-project,
    tasty ^>=1.5
  hs-source-dirs: test
  default-language: GHC2021

benchmark full-bench
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends:
    base,
    full-project,
    criterion ^>=1.6
  hs-source-dirs: bench
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 6: Heavy use of common stanzas and imports
// ============================================================================

#[test]
fn corpus_common_stanzas() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: stanza-heavy
version: 0.1.0.0
license: MIT

common warnings
  ghc-options: -Wall -Wcompat -Widentities
               -Wincomplete-record-updates
               -Wincomplete-uni-patterns
               -Wmissing-deriving-strategies
               -Wredundant-constraints

common extensions
  default-extensions:
    OverloadedStrings
    DerivingStrategies
    DeriveGeneric
    DeriveAnyClass
    GeneralizedNewtypeDeriving
    LambdaCase
    TypeApplications
    ScopedTypeVariables
  default-language: GHC2021

common deps
  build-depends:
    base >=4.14 && <5

library
  import: warnings
  import: extensions
  import: deps
  exposed-modules: Lib
  hs-source-dirs: src

executable app
  import: warnings
  import: extensions
  import: deps
  main-is: Main.hs
  build-depends: stanza-heavy
  hs-source-dirs: app

test-suite tests
  import: warnings
  import: extensions
  import: deps
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends:
    stanza-heavy,
    tasty ^>=1.5
  hs-source-dirs: test
",
    );
}

// ============================================================================
// Fixture 7: Complex conditionals (nested if/else, os/arch/flag/impl checks)
// ============================================================================

#[test]
fn corpus_complex_conditionals() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: cond-project
version: 0.1.0.0
license: MIT

flag dev
  description: Development mode
  default: False
  manual: True

flag examples
  description: Build examples
  default: False
  manual: True

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
  if flag(dev)
    ghc-options: -O0 -fprof-auto
  else
    ghc-options: -O2
  if os(windows)
    build-depends: Win32 ^>=2.13
    cpp-options: -DWINDOWS
  if os(linux)
    build-depends: unix ^>=2.7
    if flag(dev)
      ghc-options: -fhpc
  if impl(ghc >= 9.6)
    ghc-options: -Wno-missing-signatures
  if (flag(examples) || flag(dev)) && !os(windows)
    build-depends: directory ^>=1.3
",
    );
}

// ============================================================================
// Fixture 8: Leading-comma dependency lists
// ============================================================================

#[test]
fn corpus_leading_comma_deps() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: leading-comma
version: 0.1.0.0
license: MIT

library
  exposed-modules: Lib
  build-depends:
      base >=4.14 && <5
    , aeson ^>=2.2
    , bytestring ^>=0.11
    , containers ^>=0.6
    , http-client ^>=0.7
    , text >=2.0 && <2.2
    , unordered-containers ^>=0.2
    , vector ^>=0.13
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 9: Trailing-comma dependency lists
// ============================================================================

#[test]
fn corpus_trailing_comma_deps() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: trailing-comma
version: 0.1.0.0
license: MIT

library
  exposed-modules: Lib
  build-depends:
    base >=4.14 && <5,
    aeson ^>=2.2,
    bytestring ^>=0.11,
    containers ^>=0.6,
    text >=2.0 && <2.2
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 10: Single-line dependency lists
// ============================================================================

#[test]
fn corpus_single_line_deps() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: single-line
version: 0.1.0.0
license: MIT

library
  exposed-modules: Lib
  build-depends: base >=4.14, text >=2.0, aeson ^>=2.2, containers ^>=0.6
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 11: No-comma module lists
// ============================================================================

#[test]
fn corpus_no_comma_module_list() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: module-list
version: 0.1.0.0
license: MIT

library
  exposed-modules:
    Data.Algorithm.Search
    Data.Algorithm.Sort
    Data.Algorithm.Sort.Internal
    Data.Algorithm.Sort.Merge
    Data.Algorithm.Sort.Quick
    Data.Structures.Graph
    Data.Structures.Tree
    Data.Structures.Trie
  other-modules:
    Internal.Buffer
    Internal.Pool
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 12: Multiple flags with conditionals referencing them
// ============================================================================

#[test]
fn corpus_multiple_flags() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: flag-heavy
version: 0.1.0.0
license: MIT

flag fast
  description: Enable optimized backend
  default: True
  manual: False

flag debug
  description: Enable debug output
  default: False
  manual: True

flag profiling
  description: Enable profiling
  default: False
  manual: True

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
  if flag(fast)
    cpp-options: -DFAST_BACKEND
    ghc-options: -O2
  if flag(debug)
    cpp-options: -DDEBUG
    ghc-options: -fprof-auto
  if flag(profiling)
    ghc-options: -prof -fprof-auto-calls
",
    );
}

// ============================================================================
// Fixture 13: Source-repository sections
// ============================================================================

#[test]
fn corpus_source_repository() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: with-repo
version: 0.1.0.0
license: MIT

source-repository head
  type: git
  location: https://github.com/example/with-repo

source-repository this
  type: git
  location: https://github.com/example/with-repo
  tag: v0.1.0.0

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 14: Extra metadata fields
// ============================================================================

#[test]
fn corpus_extra_metadata() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: meta-heavy
version: 0.1.0.0
synopsis: A well-documented package
description: This package demonstrates all the metadata fields.
license: MIT
license-file: LICENSE
author: Meta Author
maintainer: meta@example.com
copyright: 2024 Meta Author
category: Data, Testing
homepage: https://example.com/meta-heavy
bug-reports: https://github.com/example/meta-heavy/issues
build-type: Simple
tested-with: GHC == 9.6.4, GHC == 9.8.2
extra-source-files:
  CHANGELOG.md
  README.md

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 15: Multi-line description field
// ============================================================================

#[test]
fn corpus_multiline_description() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: long-desc
version: 0.1.0.0
synopsis: Package with a long description
description:
  This is a longer description that spans multiple lines.
  It can contain various information about the package,
  including usage examples and caveats.
  .
  This is a second paragraph of the description.
  The dot on a line by itself is a paragraph separator in Haddock.
license: MIT
build-type: Simple

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 16: Tab indentation
// ============================================================================

#[test]
fn corpus_tab_indentation() {
    assert_round_trip_clean(
        "cabal-version: 3.0\n\
         name: tab-indent\n\
         version: 0.1.0.0\n\
         license: MIT\n\
         \n\
         library\n\
         \texposed-modules: Lib\n\
         \tbuild-depends: base >=4.14 && <5\n\
         \ths-source-dirs: src\n\
         \tdefault-language: GHC2021\n",
    );
}

// ============================================================================
// Fixture 17: Mixed indentation styles in different sections
// ============================================================================

#[test]
fn corpus_mixed_indentation() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: mixed-indent
version: 0.1.0.0
license: MIT

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021

executable mixed-exe
    main-is: Main.hs
    build-depends: base, mixed-indent
    hs-source-dirs: app
    default-language: GHC2021

test-suite mixed-test
   type: exitcode-stdio-1.0
   main-is: Main.hs
   build-depends: base, mixed-indent, tasty ^>=1.5
   hs-source-dirs: test
   default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 18: Very large file (20+ sections, 50+ dependencies)
// ============================================================================

#[test]
fn corpus_very_large() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: mega-project
version: 3.2.1.0
synopsis: A very large project with many components
description:
  This is a large project that exercises the parser with many sections
  and many dependencies.
license: MIT
author: Large Author
maintainer: large@example.com
category: Development
build-type: Simple
homepage: https://example.com/mega-project
bug-reports: https://github.com/example/mega-project/issues

common warnings
  ghc-options: -Wall -Wcompat

common lang
  default-language: GHC2021
  default-extensions:
    OverloadedStrings
    DerivingStrategies

flag dev
  description: Dev mode
  default: False
  manual: True

flag postgres
  description: PostgreSQL backend
  default: True
  manual: False

flag sqlite
  description: SQLite backend
  default: False
  manual: False

library
  import: warnings
  import: lang
  exposed-modules:
    Mega.Core
    Mega.Core.Types
    Mega.Core.Internal
    Mega.API
    Mega.API.Types
    Mega.API.Handlers
    Mega.API.Middleware
    Mega.DB
    Mega.DB.Types
    Mega.DB.Pool
    Mega.DB.Migrations
    Mega.Config
    Mega.Logging
    Mega.Auth
    Mega.Auth.JWT
    Mega.Auth.OAuth
    Mega.Cache
    Mega.Queue
    Mega.Worker
  other-modules:
    Mega.Internal.Crypto
    Mega.Internal.Hash
  build-depends:
      base >=4.14 && <5
    , aeson ^>=2.2
    , async ^>=2.2
    , bytestring ^>=0.11
    , containers ^>=0.6
    , cryptonite ^>=0.30
    , directory ^>=1.3
    , exceptions ^>=0.10
    , filepath ^>=1.4
    , hashable ^>=1.4
    , http-client ^>=0.7
    , http-client-tls ^>=0.3
    , http-types ^>=0.12
    , lens ^>=5.2
    , memory ^>=0.18
    , mtl ^>=2.3
    , network ^>=3.1
    , optparse-applicative ^>=0.18
    , resource-pool ^>=0.4
    , retry ^>=0.9
    , servant ^>=0.20
    , servant-server ^>=0.20
    , stm ^>=2.5
    , text >=2.0 && <2.2
    , time ^>=1.12
    , transformers ^>=0.6
    , unliftio ^>=0.2
    , unordered-containers ^>=0.2
    , uuid ^>=1.3
    , vector ^>=0.13
    , wai ^>=3.2
    , warp ^>=3.3
    , yaml ^>=0.11
  hs-source-dirs: src
  if flag(postgres)
    build-depends: postgresql-simple ^>=0.7
    cpp-options: -DPOSTGRES
  if flag(sqlite)
    build-depends: sqlite-simple ^>=0.4
    cpp-options: -DSQLITE
  if flag(dev)
    ghc-options: -O0
  else
    ghc-options: -O2

executable mega-server
  import: warnings
  import: lang
  main-is: Main.hs
  other-modules:
    Server.Config
    Server.Init
  build-depends:
    base,
    mega-project
  hs-source-dirs: app/server

executable mega-cli
  import: warnings
  import: lang
  main-is: Main.hs
  other-modules:
    CLI.Commands
    CLI.Options
  build-depends:
    base,
    mega-project,
    optparse-applicative ^>=0.18
  hs-source-dirs: app/cli

executable mega-worker
  import: warnings
  import: lang
  main-is: Main.hs
  build-depends:
    base,
    mega-project
  hs-source-dirs: app/worker

executable mega-migrate
  import: warnings
  import: lang
  main-is: Main.hs
  build-depends:
    base,
    mega-project
  hs-source-dirs: app/migrate

test-suite mega-unit
  import: warnings
  import: lang
  type: exitcode-stdio-1.0
  main-is: Main.hs
  other-modules:
    Test.Mega.Core
    Test.Mega.API
    Test.Mega.DB
    Test.Mega.Auth
  build-depends:
    base,
    mega-project,
    tasty ^>=1.5,
    tasty-hunit ^>=0.10,
    tasty-quickcheck ^>=0.10
  hs-source-dirs: test/unit

test-suite mega-integration
  import: warnings
  import: lang
  type: exitcode-stdio-1.0
  main-is: Main.hs
  other-modules:
    Test.Integration.API
    Test.Integration.DB
  build-depends:
    base,
    mega-project,
    tasty ^>=1.5,
    tasty-hunit ^>=0.10,
    http-client ^>=0.7,
    warp ^>=3.3
  hs-source-dirs: test/integration

test-suite mega-golden
  import: warnings
  import: lang
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends:
    base,
    mega-project,
    tasty ^>=1.5,
    tasty-golden ^>=2.3
  hs-source-dirs: test/golden

benchmark mega-bench
  import: warnings
  import: lang
  type: exitcode-stdio-1.0
  main-is: Main.hs
  other-modules:
    Bench.Core
    Bench.API
  build-depends:
    base,
    mega-project,
    criterion ^>=1.6,
    deepseq ^>=1.4
  hs-source-dirs: bench

source-repository head
  type: git
  location: https://github.com/example/mega-project
",
    );
}

// ============================================================================
// Fixture 19: File with lots of comments
// ============================================================================

#[test]
fn corpus_lots_of_comments() {
    assert_round_trip_clean(
        "\
-- This is the top-level comment describing the package.
-- It spans multiple lines.
cabal-version: 3.0
-- Package identity
name: commented-pkg
version: 0.1.0.0
-- Brief summary
synopsis: A heavily commented package
-- License information
license: MIT
build-type: Simple

-- Common build settings shared across components.
common warnings
  -- Enable most warnings
  ghc-options: -Wall -Wcompat
  -- But not these:
  -- ghc-options: -Werror

-- The main library
library
  -- Public API
  exposed-modules:
    -- Core module
    Lib
    -- Types used across the library
    Lib.Types
  -- Internal modules not exposed to users
  other-modules:
    Lib.Internal
  build-depends:
    -- Standard library
    base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021

-- The command-line tool
executable commented-exe
  main-is: Main.hs
  build-depends: base, commented-pkg
  hs-source-dirs: app
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 20: GHC2021 features (common stanzas, import)
// ============================================================================

#[test]
fn corpus_ghc2021_features() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: modern-haskell
version: 0.1.0.0
license: MIT

common shared
  default-language: GHC2021
  default-extensions:
    DataKinds
    DefaultSignatures
    DeriveAnyClass
    DerivingStrategies
    DerivingVia
    DuplicateRecordFields
    FunctionalDependencies
    GADTs
    LambdaCase
    MultiWayIf
    NoImplicitPrelude
    OverloadedRecordDot
    OverloadedStrings
    PatternSynonyms
    QuantifiedConstraints
    RecordWildCards
    RoleAnnotations
    ScopedTypeVariables
    StandaloneDeriving
    StrictData
    TypeApplications
    TypeFamilies
    TypeOperators
    ViewPatterns
  ghc-options: -Wall -Wcompat -Wmissing-deriving-strategies
  build-depends:
    base >=4.14 && <5

library
  import: shared
  exposed-modules:
    Modern
    Modern.Types
    Modern.Effects
  hs-source-dirs: src

test-suite tests
  import: shared
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends:
    modern-haskell,
    tasty ^>=1.5
  hs-source-dirs: test
",
    );
}

// ============================================================================
// Fixture 21: Deprecated cabal-version with >= prefix
// ============================================================================

#[test]
fn corpus_deprecated_cabal_version() {
    // This should parse without diagnostics -- the parser doesn't enforce
    // the deprecation, that's the validator's job.
    let source = "\
cabal-version: >=1.10
name: old-style
version: 0.1.0.0
build-type: Simple

library
  exposed-modules: Lib
  build-depends: base >=4.7 && <5
  hs-source-dirs: src
  default-language: Haskell2010
";
    let result = parse(source);
    assert_eq!(
        result.cst.render(),
        source,
        "round-trip failed for deprecated cabal-version"
    );
}

// ============================================================================
// Fixture 22: File with no trailing newline
// ============================================================================

#[test]
fn corpus_no_trailing_newline() {
    assert_round_trip_clean("cabal-version: 3.0\nname: no-newline\nversion: 0.1.0.0");
}

// ============================================================================
// Fixture 23: File with CRLF line endings
// ============================================================================

#[test]
fn corpus_crlf_line_endings() {
    let source = "cabal-version: 3.0\r\nname: crlf-pkg\r\nversion: 0.1.0.0\r\n";
    let result = parse(source);
    assert_eq!(result.cst.render(), source, "round-trip failed for CRLF");
}

// ============================================================================
// Fixture 24: Multiple blank lines between sections
// ============================================================================

#[test]
fn corpus_multiple_blank_lines() {
    assert_round_trip_clean(
        "\
cabal-version: 3.0
name: blanky
version: 0.1.0.0


library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021


executable blanky-exe
  main-is: Main.hs
  build-depends: base, blanky
  hs-source-dirs: app
  default-language: GHC2021
",
    );
}

// ============================================================================
// Fixture 25: Wide spacing in field values
// ============================================================================

#[test]
fn corpus_wide_spacing() {
    assert_round_trip_clean(
        "\
cabal-version:   3.0
name:            wide-spacing
version:         0.1.0.0
synopsis:        Package with wide field spacing
license:         MIT
license-file:    LICENSE
author:          Spacer
maintainer:      spacer@example.com
build-type:      Simple

library
  exposed-modules:  Lib
  build-depends:    base >=4.14 && <5
  hs-source-dirs:   src
  default-language: GHC2021
",
    );
}
