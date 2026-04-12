//! End-to-end pipeline integration tests.
//!
//! These tests exercise the full flow: init → edit → check → info, using
//! the library APIs directly (no subprocess execution).

use cabalist_opinions::lints::{run_all_lints, run_lints, LintConfig};
use cabalist_opinions::templates::{render_template, TemplateKind, TemplateVars};
use cabalist_parser::ast::derive_ast;
use cabalist_parser::edit::{self, EditBatch};
use cabalist_parser::{parse, validate};

fn default_vars() -> TemplateVars {
    TemplateVars {
        name: "pipeline-test".to_string(),
        version: "0.1.0.0".to_string(),
        synopsis: "A pipeline test project".to_string(),
        description: "Integration test for the CLI pipeline".to_string(),
        license: "MIT".to_string(),
        author: "Test Author".to_string(),
        maintainer: "test@example.com".to_string(),
        ..Default::default()
    }
}

// ============================================================================
// Init → Check pipeline: generated files are lint-clean
// ============================================================================

#[test]
fn init_then_check_all_templates_lint_clean() {
    for kind in TemplateKind::all() {
        let content = render_template(*kind, &default_vars());
        let result = parse(&content);

        // Validation should pass.
        let val_diags = validate(&result.cst);
        assert!(
            val_diags.is_empty(),
            "template {:?} failed validation: {:?}",
            kind,
            val_diags
        );

        // Lints should produce no errors (warnings for internal deps without
        // bounds are acceptable — templates reference the project itself as a
        // dep in executables, which correctly has no version constraint).
        let ast = derive_ast(&result.cst);
        let lints = run_lints(&ast, &LintConfig::default());
        let errors: Vec<_> = lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "template {:?} has lint errors: {:?}",
            kind,
            errors.iter().map(|l| l.id).collect::<Vec<_>>()
        );
    }
}

// ============================================================================
// Init → Add dep → Check pipeline
// ============================================================================

#[test]
fn init_add_dep_then_check() {
    let content = render_template(TemplateKind::LibAndExe, &default_vars());
    let result = parse(&content);
    let cst = &result.cst;

    // Find the library section and its build-depends.
    let section_id =
        edit::find_section(cst, "library", None).expect("template should have a library");
    let field_id = edit::find_field(cst, section_id, "build-depends")
        .expect("library should have build-depends");

    // Add aeson with PVP bounds.
    let edits = edit::add_list_item(cst, field_id, "aeson ^>=2.2", true);
    assert!(!edits.is_empty(), "should produce edits for adding aeson");

    let mut batch = EditBatch::new();
    batch.add_all(edits);
    let new_source = batch.apply(&cst.source);

    // Re-parse and verify.
    let new_result = parse(&new_source);
    let new_ast = derive_ast(&new_result.cst);

    // aeson should be in the dependency list.
    let all_deps = new_ast.all_dependencies();
    let has_aeson = all_deps.iter().any(|d| d.package == "aeson");
    assert!(has_aeson, "aeson should be in build-depends after add");

    // Round-trip should hold.
    assert_eq!(
        new_result.cst.render(),
        new_source,
        "round-trip must hold after adding dep"
    );

    // Lints should still be clean (no new errors/warnings).
    let lints = run_lints(&new_ast, &LintConfig::default());
    let errors: Vec<_> = lints
        .iter()
        .filter(|l| l.severity == cabalist_parser::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "should have no lint errors after adding a well-bounded dep"
    );
}

// ============================================================================
// Add dep → Remove dep → verify identical to original
// ============================================================================

/// Helper: add a dep then remove it, verify the file is byte-identical.
fn add_remove_roundtrip(source: &str, dep_str: &str, dep_name: &str) {
    let result = parse(source);
    let cst = &result.cst;

    let section_id = edit::find_section(cst, "library", None).expect("should have library section");
    let field_id =
        edit::find_field(cst, section_id, "build-depends").expect("should have build-depends");

    // Add the dependency.
    let edits = edit::add_list_item(cst, field_id, dep_str, true);
    let mut batch = EditBatch::new();
    batch.add_all(edits);
    let after_add = batch.apply(&cst.source);

    // Verify it was actually added.
    let add_result = parse(&after_add);
    let add_ast = derive_ast(&add_result.cst);
    assert!(
        add_ast
            .all_dependencies()
            .iter()
            .any(|d| d.package == dep_name),
        "dep '{}' should be present after add",
        dep_name
    );

    // Remove it.
    let section_id2 = edit::find_section(&add_result.cst, "library", None).unwrap();
    let field_id2 = edit::find_field(&add_result.cst, section_id2, "build-depends").unwrap();
    let remove_edits = edit::remove_list_item(&add_result.cst, field_id2, dep_name);
    let mut batch2 = EditBatch::new();
    batch2.add_all(remove_edits);
    let after_remove = batch2.apply(&add_result.cst.source);

    // Should be identical to the original.
    assert_eq!(
        after_remove, source,
        "add then remove should produce identical output"
    );
}

#[test]
fn add_remove_roundtrip_trailing_comma() {
    let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
    base ^>=4.17,
    text ^>=2.0,
  default-language: GHC2021
";
    add_remove_roundtrip(source, "aeson ^>=2.2", "aeson");
}

#[test]
fn add_remove_roundtrip_leading_comma() {
    let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends:
      base ^>=4.17
    , text ^>=2.0
  default-language: GHC2021
";
    add_remove_roundtrip(source, "aeson ^>=2.2", "aeson");
}

#[test]
fn add_remove_roundtrip_no_comma() {
    let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules:
    Lib
    Lib.Internal
  default-language: GHC2021
";
    // Add/remove a module instead of dep (no-comma style is for modules).
    let result = parse(source);
    let cst = &result.cst;

    let section_id = edit::find_section(cst, "library", None).unwrap();
    let field_id = edit::find_field(cst, section_id, "exposed-modules").unwrap();

    let edits = edit::add_list_item(cst, field_id, "Lib.Extra", true);
    let mut batch = EditBatch::new();
    batch.add_all(edits);
    let after_add = batch.apply(&cst.source);

    assert!(after_add.contains("Lib.Extra"), "module should be added");

    let add_result = parse(&after_add);
    let section_id2 = edit::find_section(&add_result.cst, "library", None).unwrap();
    let field_id2 = edit::find_field(&add_result.cst, section_id2, "exposed-modules").unwrap();
    let remove_edits = edit::remove_list_item(&add_result.cst, field_id2, "Lib.Extra");
    let mut batch2 = EditBatch::new();
    batch2.add_all(remove_edits);
    let after_remove = batch2.apply(&add_result.cst.source);

    assert_eq!(
        after_remove, source,
        "add then remove module should produce identical output"
    );
}

#[test]
fn add_remove_roundtrip_single_line() {
    let source = "\
cabal-version: 3.0
name: test
version: 0.1

library
  exposed-modules: Lib
  build-depends: base ^>=4.17, text ^>=2.0
  default-language: GHC2021
";
    add_remove_roundtrip(source, "aeson ^>=2.2", "aeson");
}

// ============================================================================
// Lint golden tests: known messy files with expected lint IDs
// ============================================================================

#[test]
fn lint_golden_messy_no_bounds() {
    let source = "\
cabal-version: 3.0
name: messy-project
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends:
    base
    text >=2.0
    aeson
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());
    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();

    // base: no bounds at all
    assert!(ids.contains(&"wide-any-version"), "base has no bounds");
    // text: no upper bound
    assert!(
        ids.contains(&"missing-upper-bound"),
        "text has no upper bound"
    );
    // aeson: no bounds at all
    // missing metadata
    assert!(ids.contains(&"missing-synopsis"));
    assert!(ids.contains(&"missing-description"));
    assert!(ids.contains(&"missing-source-repo"));
    assert!(ids.contains(&"missing-bug-reports"));
}

#[test]
fn lint_golden_werror_and_low_cabal_version() {
    let source = "\
cabal-version: 2.4
name: old-project
version: 0.1.0.0
synopsis: An old project
description: An old project
bug-reports: https://github.com/example/old

source-repository head
  type: git
  location: https://github.com/example/old

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  ghc-options: -Wall -Werror
  default-language: Haskell2010
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());
    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();

    assert!(ids.contains(&"cabal-version-low"));
    assert!(ids.contains(&"ghc-options-werror"));
    // Should NOT have missing-synopsis etc since they're present.
    assert!(!ids.contains(&"missing-synopsis"));
    assert!(!ids.contains(&"missing-source-repo"));
}

#[test]
fn lint_golden_unused_flag_and_duplicate_dep() {
    let source = "\
cabal-version: 3.0
name: flag-test
version: 0.1.0.0
synopsis: Test
description: Test

source-repository head
  type: git
  location: https://example.com

flag dev
  description: Development mode
  default: False

library
  exposed-modules: Lib
  build-depends:
    base ^>=4.17
    , text ^>=2.0
    , text ^>=2.0
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());
    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();

    assert!(ids.contains(&"unused-flag"), "dev flag is never used");
    assert!(ids.contains(&"duplicate-dep"), "text appears twice");
}

#[test]
fn lint_golden_no_common_stanza_suggestion() {
    let source = "\
cabal-version: 3.0
name: dup-test
version: 0.1.0.0
synopsis: Test
description: Test
bug-reports: https://example.com

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  ghc-options: -Wall -Wcompat
  default-extensions: OverloadedStrings
  default-language: GHC2021
  hs-source-dirs: src

executable my-exe
  main-is: Main.hs
  build-depends: base ^>=4.17, dup-test
  ghc-options: -Wall -Wcompat
  default-extensions: OverloadedStrings
  default-language: GHC2021
  hs-source-dirs: app

test-suite my-tests
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base ^>=4.17, dup-test
  ghc-options: -Wall -Wcompat
  default-extensions: OverloadedStrings
  default-language: GHC2021
  hs-source-dirs: test
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());
    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();

    assert!(
        ids.contains(&"no-common-stanza"),
        "3 components share 5+ fields, should suggest common stanza"
    );
}

#[test]
fn lint_golden_exposed_no_modules() {
    let source = "\
cabal-version: 3.0
name: empty-lib
version: 0.1.0.0
synopsis: Test
description: Test
bug-reports: https://example.com

source-repository head
  type: git
  location: https://example.com

library
  build-depends: base ^>=4.17
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());
    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();

    assert!(
        ids.contains(&"exposed-no-modules"),
        "library with no exposed-modules should be flagged"
    );
}

// ============================================================================
// Filesystem lint integration: string-gaps with temp dirs
// ============================================================================

#[test]
fn string_gaps_integration_missing_source_dir() {
    let tmp = std::env::temp_dir().join("cabalist-pipeline-string-gaps");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let source = "\
cabal-version: 3.0
name: gaps-test
version: 0.1.0.0

library
  exposed-modules: Lib
  hs-source-dirs: src
  default-language: GHC2021
";
    // src/ does not exist in tmp.
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_all_lints(&ast, &LintConfig::default(), &tmp);
    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();

    assert!(
        ids.contains(&"string-gaps"),
        "should detect missing src/ directory"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn string_gaps_integration_all_files_present() {
    let tmp = std::env::temp_dir().join("cabalist-pipeline-string-gaps-ok");
    let _ = std::fs::remove_dir_all(&tmp);
    let src = tmp.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("Lib.hs"), "module Lib where\n").unwrap();

    let source = "\
cabal-version: 3.0
name: gaps-test
version: 0.1.0.0

library
  exposed-modules: Lib
  hs-source-dirs: src
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_all_lints(&ast, &LintConfig::default(), &tmp);
    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();

    assert!(
        !ids.contains(&"string-gaps"),
        "all files present, should not fire"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

// ============================================================================
// Check exit code semantics
// ============================================================================

#[test]
fn check_exit_code_clean() {
    let source = "\
cabal-version: 3.0
name: clean
version: 0.1.0.0
synopsis: Clean
description: Clean project
bug-reports: https://example.com

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends: base ^>=4.17
  default-language: GHC2021
";
    let result = parse(source);
    let val_diags = validate(&result.cst);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());

    let errors = val_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Error)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Error)
            .count();
    let warnings = val_diags
        .iter()
        .filter(|d| d.severity == cabalist_parser::Severity::Warning)
        .count()
        + lints
            .iter()
            .filter(|l| l.severity == cabalist_parser::Severity::Warning)
            .count();

    assert_eq!(errors, 0, "clean file should have no errors");
    assert_eq!(warnings, 0, "clean file should have no warnings");
}

#[test]
fn check_exit_code_warnings() {
    let source = "\
cabal-version: 3.0
name: warn-test
version: 0.1.0.0
synopsis: Test
description: Test
bug-reports: https://example.com

source-repository head
  type: git
  location: https://example.com

library
  exposed-modules: Lib
  build-depends: base >=4.14
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());

    let warnings = lints
        .iter()
        .filter(|l| l.severity == cabalist_parser::Severity::Warning)
        .count();
    assert!(warnings > 0, "should have warnings (missing upper bound)");
}
