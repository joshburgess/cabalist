//! Regression tests for lint correctness.
//!
//! These tests verify that specific bugs stay fixed:
//! - Lint spans point to actual source locations when CST is provided
//! - Repeatable fields (build-depends, etc.) don't trigger false duplicate warnings
//! - Self-dependencies are not flagged by version bound lints

use cabalist_opinions::lints::{run_lints_with_cst, Lint, LintConfig};
use cabalist_parser::ast::derive_ast;
use cabalist_parser::parse;
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/cabalist-opinions -> crates
    path.pop(); // crates -> workspace root
    path.push("tests");
    path.push("fixtures");
    path.push("real-world");
    path
}

fn lint_with_cst(source: &str) -> Vec<Lint> {
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    run_lints_with_cst(&ast, Some(&result.cst), &LintConfig::default())
}

// ============================================================================
// Lint spans must resolve to actual source positions
// ============================================================================

#[test]
fn lint_spans_point_to_source_locations() {
    let source = "\
cabal-version: 3.0
name: span-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base >=4.14
";
    let lints = lint_with_cst(source);
    // This file should trigger missing-upper-bound on base.
    let upper_bound_lint = lints
        .iter()
        .find(|l| l.id == "missing-upper-bound")
        .expect("should fire missing-upper-bound");

    // Span must not be 0:0 — it should point into the source.
    assert!(
        upper_bound_lint.span.start > 0,
        "lint span should point to the dependency, not file start. Got span: {:?}",
        upper_bound_lint.span
    );
    assert!(
        upper_bound_lint.span.start < source.len(),
        "lint span should be within source bounds"
    );
}

#[test]
fn lint_spans_nonzero_on_real_world_files() {
    let fixtures = fixtures_dir();
    if !fixtures.exists() {
        return; // Skip if fixtures aren't available.
    }

    for entry in fs::read_dir(&fixtures).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "cabal") {
            continue;
        }

        let source = fs::read_to_string(&path).unwrap();
        let result = parse(&source);
        let ast = derive_ast(&result.cst);
        let lints = run_lints_with_cst(&ast, Some(&result.cst), &LintConfig::default());

        for lint in &lints {
            // Lints without a specific node (package-level) may legitimately
            // use span 0, but lints attached to a dependency or component
            // should have a non-zero span.
            if lint.id == "missing-upper-bound"
                || lint.id == "missing-lower-bound"
                || lint.id == "wide-any-version"
                || lint.id == "duplicate-dep"
            {
                assert!(
                    lint.span.start > 0,
                    "{}: lint '{}' on {} should have a non-zero span, got {:?}",
                    path.display(),
                    lint.id,
                    lint.message,
                    lint.span
                );
            }

            // All spans should be within source bounds.
            assert!(
                lint.span.start <= source.len(),
                "{}: lint '{}' span start {} exceeds source length {}",
                path.display(),
                lint.id,
                lint.span.start,
                source.len()
            );
        }
    }
}

// ============================================================================
// Self-dependencies must not trigger version bound lints
// ============================================================================

#[test]
fn self_dep_not_flagged_across_components() {
    let source = "\
cabal-version: 3.0
name: my-lib
version: 0.1.0.0
synopsis: test

library
  exposed-modules: MyLib
  build-depends: base ^>=4.17
  default-language: GHC2021

executable my-exe
  main-is: Main.hs
  build-depends: base ^>=4.17, my-lib
  default-language: GHC2021

test-suite my-tests
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base ^>=4.17, my-lib
  default-language: GHC2021

benchmark my-bench
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base ^>=4.17, my-lib
  default-language: GHC2021
";
    let lints = lint_with_cst(source);

    for lint in &lints {
        assert!(
            !lint.message.contains("'my-lib'"),
            "self-dependency 'my-lib' should not trigger lint '{}': {}",
            lint.id,
            lint.message
        );
    }
}

// ============================================================================
// Repeatable fields must not trigger duplicate field validation warnings
// ============================================================================

#[test]
fn repeatable_fields_no_duplicate_warning() {
    let source = "\
cabal-version: 3.0
name: multi-deps
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  build-depends: text ^>=2.0
  build-depends: aeson ^>=2.2
  ghc-options: -Wall
  ghc-options: -Wcompat
  default-extensions: OverloadedStrings
  default-extensions: DerivingStrategies
  other-modules: Internal.Foo
  other-modules: Internal.Bar
  default-language: GHC2021
";
    let result = parse(source);
    let diags = cabalist_parser::validate(&result.cst);

    for diag in &diags {
        assert!(
            !diag.message.contains("duplicate field: `build-depends`"),
            "build-depends should be repeatable: {}", diag.message
        );
        assert!(
            !diag.message.contains("duplicate field: `ghc-options`"),
            "ghc-options should be repeatable: {}", diag.message
        );
        assert!(
            !diag.message.contains("duplicate field: `default-extensions`"),
            "default-extensions should be repeatable: {}", diag.message
        );
        assert!(
            !diag.message.contains("duplicate field: `other-modules`"),
            "other-modules should be repeatable: {}", diag.message
        );
    }
}

#[test]
fn non_repeatable_fields_still_flagged() {
    let source = "\
cabal-version: 3.0
name: dup-test
version: 0.1.0.0

library
  exposed-modules: Lib
  default-language: Haskell2010
  default-language: GHC2021
";
    let result = parse(source);
    let diags = cabalist_parser::validate(&result.cst);

    assert!(
        diags.iter().any(|d| d.message.contains("duplicate field: `default-language`")),
        "non-repeatable field should still be flagged as duplicate"
    );
}

#[test]
fn real_world_files_no_false_duplicate_warnings() {
    let fixtures = fixtures_dir();
    if !fixtures.exists() {
        return;
    }

    let repeatable = [
        "build-depends",
        "exposed-modules",
        "other-modules",
        "default-extensions",
        "other-extensions",
        "ghc-options",
        "hs-source-dirs",
        "build-tool-depends",
        "build-tools",
        "mixins",
        "pkgconfig-depends",
        "extra-libraries",
        "c-sources",
        "cxx-sources",
        "js-sources",
    ];

    for entry in fs::read_dir(&fixtures).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "cabal") {
            continue;
        }

        let source = fs::read_to_string(&path).unwrap();
        let result = parse(&source);
        let diags = cabalist_parser::validate(&result.cst);

        for diag in &diags {
            for field in &repeatable {
                assert!(
                    !diag.message.contains(&format!("duplicate field: `{field}`")),
                    "{}: false positive duplicate warning for repeatable field '{field}': {}",
                    path.display(),
                    diag.message
                );
            }
        }
    }
}
