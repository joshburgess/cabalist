//! Validation integration tests.
//!
//! Tests the spec-level validation on intentionally problematic files to verify
//! that the correct diagnostics are emitted.

use cabalist_parser::diagnostic::Severity;
use cabalist_parser::{parse, validate};

/// Parse and validate, returning diagnostics.
fn validate_source(source: &str) -> Vec<cabalist_parser::Diagnostic> {
    let result = parse(source);
    validate(&result.cst)
}

/// Check that a diagnostic with a given message substring exists.
fn has_diagnostic(diags: &[cabalist_parser::Diagnostic], substring: &str) -> bool {
    diags.iter().any(|d| d.message.contains(substring))
}

/// Count diagnostics by severity.
fn count_severity(diags: &[cabalist_parser::Diagnostic], severity: Severity) -> usize {
    diags.iter().filter(|d| d.severity == severity).count()
}

// ============================================================================
// Missing required fields
// ============================================================================

#[test]
fn validation_missing_all_required_fields() {
    let diags = validate_source("");
    assert!(has_diagnostic(
        &diags,
        "missing required field: `cabal-version`"
    ));
    assert!(has_diagnostic(&diags, "missing required field: `name`"));
    assert!(has_diagnostic(&diags, "missing required field: `version`"));
}

#[test]
fn validation_missing_name() {
    let diags = validate_source("cabal-version: 3.0\nversion: 0.1.0.0\n");
    assert!(has_diagnostic(&diags, "missing required field: `name`"));
    assert!(!has_diagnostic(&diags, "missing required field: `version`"));
    assert!(!has_diagnostic(
        &diags,
        "missing required field: `cabal-version`"
    ));
}

#[test]
fn validation_missing_version() {
    let diags = validate_source("cabal-version: 3.0\nname: foo\n");
    assert!(has_diagnostic(&diags, "missing required field: `version`"));
    assert!(!has_diagnostic(&diags, "missing required field: `name`"));
}

#[test]
fn validation_all_required_present_no_errors() {
    let diags = validate_source("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n");
    assert!(!has_diagnostic(&diags, "missing required field"));
}

// ============================================================================
// Duplicate fields
// ============================================================================

#[test]
fn validation_duplicate_top_level_field() {
    let diags = validate_source("cabal-version: 3.0\nname: foo\nname: bar\nversion: 0.1.0.0\n");
    assert!(has_diagnostic(&diags, "duplicate field: `name`"));
}

#[test]
fn validation_duplicate_field_in_section() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  default-language: Haskell2010
  default-language: GHC2021
";
    let diags = validate_source(src);
    assert!(has_diagnostic(
        &diags,
        "duplicate field: `default-language`"
    ));
}

#[test]
fn validation_repeatable_field_not_flagged() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  build-depends: base
  build-depends: text
";
    let diags = validate_source(src);
    assert!(
        !has_diagnostic(&diags, "duplicate field"),
        "build-depends should be allowed multiple times: {diags:?}"
    );
}

#[test]
fn validation_duplicate_field_case_insensitive() {
    let diags = validate_source("cabal-version: 3.0\nName: foo\nname: bar\nversion: 0.1.0.0\n");
    assert!(has_diagnostic(&diags, "duplicate field: `name`"));
}

#[test]
fn validation_duplicate_field_underscore_hyphen() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo
  default-language: Haskell2010
  default_language: GHC2021
";
    let diags = validate_source(src);
    assert!(has_diagnostic(
        &diags,
        "duplicate field: `default-language`"
    ));
}

// ============================================================================
// Duplicate sections
// ============================================================================

#[test]
fn validation_duplicate_unnamed_library() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Foo

library
  exposed-modules: Bar
";
    let diags = validate_source(src);
    assert!(has_diagnostic(&diags, "duplicate section: `library`"));
}

#[test]
fn validation_duplicate_named_executable() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

executable bar
  main-is: Main.hs

executable bar
  main-is: Other.hs
";
    let diags = validate_source(src);
    assert!(has_diagnostic(
        &diags,
        "duplicate section: `executable bar`"
    ));
}

#[test]
fn validation_different_exe_names_no_duplicate() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

executable bar
  main-is: Main.hs

executable baz
  main-is: Other.hs
";
    let diags = validate_source(src);
    assert!(!has_diagnostic(&diags, "duplicate section"));
}

// ============================================================================
// Missing common stanza references
// ============================================================================

#[test]
fn validation_import_references_missing_stanza() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  import: nonexistent
  exposed-modules: Foo
";
    let diags = validate_source(src);
    assert!(has_diagnostic(
        &diags,
        "import references undefined common stanza: `nonexistent`"
    ));
}

#[test]
fn validation_import_references_existing_stanza_ok() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

common shared
  ghc-options: -Wall

library
  import: shared
  exposed-modules: Foo
";
    let diags = validate_source(src);
    assert!(!has_diagnostic(&diags, "import references undefined"));
}

// ============================================================================
// Invalid build-type
// ============================================================================

#[test]
fn validation_invalid_build_type() {
    let diags =
        validate_source("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\nbuild-type: Bogus\n");
    assert!(has_diagnostic(
        &diags,
        "invalid `build-type` value: `Bogus`"
    ));
}

#[test]
fn validation_valid_build_types() {
    for bt in &["Simple", "Configure", "Make", "Custom"] {
        let src = format!("cabal-version: 3.0\nname: foo\nversion: 0.1.0.0\nbuild-type: {bt}\n");
        let diags = validate_source(&src);
        assert!(
            !has_diagnostic(&diags, "invalid `build-type`"),
            "build-type '{bt}' should be valid"
        );
    }
}

// ============================================================================
// Library without exposed-modules
// ============================================================================

#[test]
fn validation_library_no_exposed_modules() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  build-depends: base
";
    let diags = validate_source(src);
    assert!(has_diagnostic(
        &diags,
        "library section has no `exposed-modules`"
    ));
}

#[test]
fn validation_library_with_exposed_modules_ok() {
    let src = "\
cabal-version: 3.0
name: foo
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base
";
    let diags = validate_source(src);
    assert!(!has_diagnostic(&diags, "exposed-modules"));
}

// ============================================================================
// cabal-version should be first field
// ============================================================================

#[test]
fn validation_cabal_version_not_first() {
    let diags = validate_source("name: foo\ncabal-version: 3.0\nversion: 0.1.0.0\n");
    assert!(has_diagnostic(
        &diags,
        "`cabal-version` should be the first field"
    ));
}

#[test]
fn validation_cabal_version_first_after_comments_ok() {
    let diags = validate_source("-- comment\ncabal-version: 3.0\nname: foo\nversion: 0.1.0.0\n");
    assert!(!has_diagnostic(&diags, "should be the first field"));
}

// ============================================================================
// Invalid cabal-version value
// ============================================================================

#[test]
fn validation_cabal_version_invalid_value() {
    let diags = validate_source("cabal-version: foobar\nname: foo\nversion: 0.1.0.0\n");
    assert!(has_diagnostic(&diags, "unrecognized `cabal-version` value"));
}

#[test]
fn validation_cabal_version_valid_with_prefix() {
    let diags = validate_source("cabal-version: >=1.10\nname: foo\nversion: 0.1.0.0\n");
    assert!(!has_diagnostic(&diags, "unrecognized `cabal-version`"));
}

// ============================================================================
// Full valid file -- zero diagnostics
// ============================================================================

#[test]
fn validation_full_valid_file_clean() {
    let src = "\
cabal-version: 3.0
name: valid-pkg
version: 0.1.0.0
synopsis: A valid package
build-type: Simple

common shared
  ghc-options: -Wall

library
  import: shared
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  default-language: GHC2021

executable valid-exe
  import: shared
  main-is: Main.hs
  build-depends: base, valid-pkg
  default-language: GHC2021

test-suite valid-tests
  import: shared
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, valid-pkg, tasty ^>=1.5
  default-language: GHC2021
";
    let diags = validate_source(src);
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for valid file, got: {diags:?}"
    );
}

// ============================================================================
// Multiple issues in one file
// ============================================================================

#[test]
fn validation_multiple_issues() {
    let src = "\
name: foo
version: 0.1.0.0

library
  default-language: Haskell2010
  default-language: GHC2021

executable bar
  main-is: Main.hs

executable bar
  main-is: Other.hs
";
    let diags = validate_source(src);
    // Missing cabal-version.
    assert!(has_diagnostic(
        &diags,
        "missing required field: `cabal-version`"
    ));
    // Duplicate field in library.
    assert!(has_diagnostic(
        &diags,
        "duplicate field: `default-language`"
    ));
    // Duplicate section.
    assert!(has_diagnostic(
        &diags,
        "duplicate section: `executable bar`"
    ));
    // Library without exposed-modules.
    assert!(has_diagnostic(&diags, "exposed-modules"));
    // Multiple errors expected.
    assert!(
        count_severity(&diags, Severity::Error) >= 2,
        "expected at least 2 errors"
    );
}
