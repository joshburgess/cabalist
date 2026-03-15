//! CLI integration tests.
//!
//! Tests the core library functions used by the CLI commands, exercised
//! directly (without subprocess execution) to keep tests fast and networkless.

use cabalist_opinions::lints::{run_lints, LintConfig};
use cabalist_opinions::templates::{render_template, TemplateKind, TemplateVars};
use cabalist_parser::ast::derive_ast;
use cabalist_parser::{parse, validate};

// ============================================================================
// Init: generated .cabal files parse cleanly
// ============================================================================

fn default_vars() -> TemplateVars {
    TemplateVars {
        name: "test-project".to_string(),
        version: "0.1.0.0".to_string(),
        synopsis: "A test project".to_string(),
        description: "A test project for integration tests".to_string(),
        license: "MIT".to_string(),
        author: "Test Author".to_string(),
        maintainer: "test@example.com".to_string(),
        ..Default::default()
    }
}

#[test]
fn init_library_template_parses_cleanly() {
    let content = render_template(TemplateKind::Library, &default_vars());
    let result = parse(&content);
    assert!(
        result.diagnostics.is_empty(),
        "library template should parse without diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.cst.render(), content, "round-trip must hold");
}

#[test]
fn init_application_template_parses_cleanly() {
    let content = render_template(TemplateKind::Application, &default_vars());
    let result = parse(&content);
    assert!(
        result.diagnostics.is_empty(),
        "application template should parse without diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.cst.render(), content, "round-trip must hold");
}

#[test]
fn init_lib_and_exe_template_parses_cleanly() {
    let content = render_template(TemplateKind::LibAndExe, &default_vars());
    let result = parse(&content);
    assert!(
        result.diagnostics.is_empty(),
        "lib-and-exe template should parse without diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.cst.render(), content, "round-trip must hold");
}

#[test]
fn init_full_template_parses_cleanly() {
    let content = render_template(TemplateKind::Full, &default_vars());
    let result = parse(&content);
    assert!(
        result.diagnostics.is_empty(),
        "full template should parse without diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.cst.render(), content, "round-trip must hold");
}

// ============================================================================
// Init: generated templates pass validation
// ============================================================================

#[test]
fn init_templates_pass_validation() {
    for kind in TemplateKind::all() {
        let content = render_template(*kind, &default_vars());
        let result = parse(&content);
        let diags = validate(&result.cst);
        assert!(
            diags.is_empty(),
            "template {:?} should pass validation, got: {:?}",
            kind,
            diags
        );
    }
}

// ============================================================================
// Init: generated templates contain expected metadata
// ============================================================================

#[test]
fn init_templates_contain_metadata() {
    for kind in TemplateKind::all() {
        let content = render_template(*kind, &default_vars());
        let result = parse(&content);
        let ast = derive_ast(&result.cst);

        assert_eq!(
            ast.name,
            Some("test-project"),
            "template {:?} should have correct name",
            kind
        );
        assert!(
            ast.version.is_some(),
            "template {:?} should have version",
            kind
        );
        assert_eq!(
            ast.license,
            Some("MIT"),
            "template {:?} should have license",
            kind
        );
    }
}

// ============================================================================
// Check: finds expected lints in a test fixture
// ============================================================================

#[test]
fn check_finds_expected_lints() {
    let source = "\
cabal-version: 3.0
name: check-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base >=4.14
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);
    let lints = run_lints(&ast, &LintConfig::default());

    let ids: Vec<&str> = lints.iter().map(|l| l.id).collect();
    // Missing upper bound on base.
    assert!(ids.contains(&"missing-upper-bound"));
    // Missing synopsis.
    assert!(ids.contains(&"missing-synopsis"));
    // Missing default-language.
    assert!(ids.contains(&"missing-default-language"));
}

// ============================================================================
// Check: validation catches spec violations
// ============================================================================

#[test]
fn check_validation_catches_spec_violations() {
    let source = "\
name: missing-cv
version: 0.1.0.0

library
  build-depends: base
";
    let result = parse(source);
    let diags = validate(&result.cst);

    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("missing required field: `cabal-version`")),
        "should catch missing cabal-version"
    );
}

// ============================================================================
// Info: AST derivation produces expected component structure
// ============================================================================

#[test]
fn info_component_structure() {
    let source = "\
cabal-version: 3.0
name: info-test
version: 0.1.0.0
synopsis: An info test
license: MIT
author: Author
maintainer: author@example.com

library
  exposed-modules:
    Info.Core
    Info.Types
  build-depends: base ^>=4.17, text ^>=2.0
  hs-source-dirs: src
  default-language: GHC2021

executable info-exe
  main-is: Main.hs
  build-depends: base, info-test
  hs-source-dirs: app
  default-language: GHC2021

test-suite info-tests
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, info-test, tasty ^>=1.5
  hs-source-dirs: test
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    // Basic metadata.
    assert_eq!(ast.name, Some("info-test"));
    assert_eq!(ast.synopsis, Some("An info test"));
    assert_eq!(ast.license, Some("MIT"));

    // Components.
    let comps = ast.all_components();
    assert_eq!(comps.len(), 3, "should have 3 components");

    // Library details.
    let lib = ast.library.as_ref().expect("should have library");
    assert_eq!(lib.exposed_modules.len(), 2);
    assert_eq!(lib.fields.build_depends.len(), 2);

    // Executable.
    assert_eq!(ast.executables.len(), 1);
    assert_eq!(ast.executables[0].fields.name, Some("info-exe"));

    // Test suite.
    assert_eq!(ast.test_suites.len(), 1);
    assert_eq!(ast.test_suites[0].fields.name, Some("info-tests"));
}
