//! Lint integration tests for cabalist-opinions.
//!
//! Tests lints on realistic .cabal files to verify that the right lints fire
//! (or don't fire) in realistic scenarios.

use cabalist_opinions::lints::{run_lints, Lint, LintConfig};
use cabalist_parser::ast::derive_ast;
use cabalist_parser::diagnostic::Severity;
use cabalist_parser::parse;

/// Parse and lint with default config.
fn parse_and_lint(source: &str) -> Vec<Lint> {
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    run_lints(&ast, &LintConfig::default())
}

/// Parse and lint with a custom config.
fn parse_and_lint_with_config(source: &str, config: &LintConfig) -> Vec<Lint> {
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    run_lints(&ast, config)
}

/// Collect lint IDs from a vector of lints.
fn lint_ids(lints: &[Lint]) -> Vec<&str> {
    lints.iter().map(|l| l.id).collect()
}

// ============================================================================
// Well-formed file with proper bounds -- zero warnings
// ============================================================================

#[test]
fn lint_well_formed_file_zero_warnings() {
    let source = "\
cabal-version: 3.0
name: clean-pkg
version: 0.1.0.0
synopsis: A clean package
description: A well-formed package with proper bounds
bug-reports: https://github.com/example/clean-pkg/issues
build-type: Simple

source-repository head
  type: git
  location: https://github.com/example/clean-pkg

common shared
  ghc-options: -Wall -Wcompat
  default-language: GHC2021

library
  import: shared
  exposed-modules: Lib
  build-depends:
      base >=4.14 && <5
    , aeson ^>=2.2
    , text >=2.0 && <2.2
  hs-source-dirs: src
  default-language: GHC2021

executable clean-exe
  import: shared
  main-is: Main.hs
  build-depends:
    base >=4.14 && <5,
    clean-pkg ^>=0.1
  hs-source-dirs: app
  default-language: GHC2021

test-suite clean-tests
  import: shared
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends:
    base >=4.14 && <5,
    clean-pkg ^>=0.1,
    tasty ^>=1.5
  hs-source-dirs: test
  default-language: GHC2021
";
    let lints = parse_and_lint(source);

    // Filter out info-level lints (missing-description is info, not warning).
    let warnings_and_errors: Vec<_> = lints
        .iter()
        .filter(|l| l.severity == Severity::Warning || l.severity == Severity::Error)
        .collect();
    assert!(
        warnings_and_errors.is_empty(),
        "expected zero warnings/errors for well-formed file, got: {:?}",
        warnings_and_errors
    );
}

// ============================================================================
// Missing upper bounds
// ============================================================================

#[test]
fn lint_missing_upper_bound_fires() {
    let source = "\
cabal-version: 3.0
name: ub-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends:
    base >=4.14,
    text >=2.0
  default-language: GHC2021
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        ids.contains(&"missing-upper-bound"),
        "should fire missing-upper-bound"
    );
}

#[test]
fn lint_missing_upper_bound_does_not_fire_for_major_bound() {
    let source = "\
cabal-version: 3.0
name: ub-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends:
    base ^>=4.17,
    text ^>=2.0
  default-language: GHC2021
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        !ids.contains(&"missing-upper-bound"),
        "should NOT fire for ^>="
    );
}

// ============================================================================
// ghc-options -Werror
// ============================================================================

#[test]
fn lint_ghc_options_werror_fires_in_library() {
    let source = "\
cabal-version: 3.0
name: werror-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
  ghc-options: -Wall -Werror
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        ids.contains(&"ghc-options-werror"),
        "should fire ghc-options-werror"
    );
}

#[test]
fn lint_ghc_options_werror_fires_in_common_stanza() {
    let source = "\
cabal-version: 3.0
name: werror-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

common shared
  ghc-options: -Wall -Werror
  default-language: GHC2021

library
  import: shared
  exposed-modules: Lib
  build-depends: base ^>=4.17
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        ids.contains(&"ghc-options-werror"),
        "should fire ghc-options-werror for common stanza"
    );
}

// ============================================================================
// Unused flag
// ============================================================================

#[test]
fn lint_unused_flag_fires() {
    let source = "\
cabal-version: 3.0
name: flag-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

flag unused-flag
  description: An unused flag
  default: False

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(ids.contains(&"unused-flag"), "should fire unused-flag");
}

#[test]
fn lint_used_flag_does_not_fire() {
    let source = "\
cabal-version: 3.0
name: flag-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

flag dev
  description: Dev mode
  default: False

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
  if flag(dev)
    ghc-options: -O0
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        !ids.contains(&"unused-flag"),
        "should NOT fire for used flag"
    );
}

// ============================================================================
// Missing metadata lints
// ============================================================================

#[test]
fn lint_missing_metadata_fires() {
    let source = "\
cabal-version: 3.0
name: bare-pkg
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(ids.contains(&"missing-synopsis"));
    assert!(ids.contains(&"missing-description"));
    assert!(ids.contains(&"missing-source-repo"));
    assert!(ids.contains(&"missing-bug-reports"));
}

#[test]
fn lint_missing_default_language_fires() {
    let source = "\
cabal-version: 3.0
name: lang-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        ids.contains(&"missing-default-language"),
        "should fire missing-default-language"
    );
}

// ============================================================================
// cabal-version-low
// ============================================================================

#[test]
fn lint_cabal_version_low_fires() {
    let source = "\
cabal-version: 2.4
name: old-cabal
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: Haskell2010
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        ids.contains(&"cabal-version-low"),
        "should fire cabal-version-low for 2.4"
    );
}

#[test]
fn lint_cabal_version_not_low_for_3_0() {
    let source = "\
cabal-version: 3.0
name: modern
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(!ids.contains(&"cabal-version-low"));
}

// ============================================================================
// Duplicate dependency
// ============================================================================

#[test]
fn lint_duplicate_dep_fires() {
    let source = "\
cabal-version: 3.0
name: dup-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends:
    base ^>=4.17,
    text ^>=2.0,
    base ^>=4.17
  default-language: GHC2021
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(ids.contains(&"duplicate-dep"), "should fire duplicate-dep");
}

// ============================================================================
// wide-any-version
// ============================================================================

#[test]
fn lint_wide_any_version_fires() {
    let source = "\
cabal-version: 3.0
name: wide-test
version: 0.1.0.0
synopsis: test
description: test
bug-reports: https://example.com/issues

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends: base
  default-language: GHC2021
";
    let lints = parse_and_lint(source);
    let ids = lint_ids(&lints);
    assert!(
        ids.contains(&"wide-any-version"),
        "should fire wide-any-version for unconstrained dep"
    );
}

// ============================================================================
// Disabling lints via config
// ============================================================================

#[test]
fn lint_disabled_via_config() {
    let source = "\
cabal-version: 3.0
name: disable-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
";
    let config = LintConfig {
        disabled: vec![
            "missing-synopsis".to_string(),
            "missing-description".to_string(),
            "missing-source-repo".to_string(),
            "missing-bug-reports".to_string(),
        ],
        ..Default::default()
    };
    let lints = parse_and_lint_with_config(source, &config);
    let ids = lint_ids(&lints);
    assert!(!ids.contains(&"missing-synopsis"));
    assert!(!ids.contains(&"missing-description"));
    assert!(!ids.contains(&"missing-source-repo"));
    assert!(!ids.contains(&"missing-bug-reports"));
}

// ============================================================================
// Promoting lint to error via config
// ============================================================================

#[test]
fn lint_promoted_to_error() {
    let source = "\
cabal-version: 3.0
name: promote-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
";
    let config = LintConfig {
        errors: vec!["missing-synopsis".to_string()],
        ..Default::default()
    };
    let lints = parse_and_lint_with_config(source, &config);
    let synopsis_lint = lints.iter().find(|l| l.id == "missing-synopsis");
    assert!(synopsis_lint.is_some());
    assert_eq!(synopsis_lint.unwrap().severity, Severity::Error);
}
