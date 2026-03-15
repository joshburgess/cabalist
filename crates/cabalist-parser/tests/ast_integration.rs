//! AST derivation integration tests.
//!
//! Tests the full parse -> AST derivation pipeline on realistic files, verifying
//! that typed access (dependencies, components, metadata, etc.) works correctly.

use cabalist_parser::ast::{derive_ast, Component, Version, VersionRange};
use cabalist_parser::parse;

// ============================================================================
// Metadata extraction
// ============================================================================

#[test]
fn ast_extracts_basic_metadata() {
    let source = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0
synopsis: A test package
license: MIT
author: Test Author
maintainer: test@example.com
homepage: https://example.com
bug-reports: https://example.com/issues
category: Testing
build-type: Simple

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  hs-source-dirs: src
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    assert_eq!(ast.name, Some("my-pkg"));
    assert_eq!(
        ast.version,
        Some(Version {
            components: vec![0, 1, 0, 0]
        })
    );
    assert_eq!(ast.synopsis, Some("A test package"));
    assert_eq!(ast.license, Some("MIT"));
    assert_eq!(ast.author, Some("Test Author"));
    assert_eq!(ast.maintainer, Some("test@example.com"));
    assert_eq!(ast.homepage, Some("https://example.com"));
    assert_eq!(ast.bug_reports, Some("https://example.com/issues"));
    assert_eq!(ast.category, Some("Testing"));
    assert_eq!(ast.build_type, Some("Simple"));
    assert!(ast.cabal_version.is_some());
    assert_eq!(
        ast.cabal_version.as_ref().unwrap().version,
        Some(Version {
            components: vec![3, 0]
        })
    );
}

// ============================================================================
// all_dependencies()
// ============================================================================

#[test]
fn ast_all_dependencies_count() {
    let source = "\
cabal-version: 3.0
name: dep-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends:
      base >=4.14 && <5
    , aeson ^>=2.2
    , text >=2.0
  default-language: GHC2021

executable dep-exe
  main-is: Main.hs
  build-depends:
    base,
    dep-test,
    optparse-applicative ^>=0.18
  default-language: GHC2021

test-suite dep-test-suite
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends:
    base,
    dep-test,
    tasty ^>=1.5
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    let all_deps = ast.all_dependencies();
    // library: base, aeson, text (3)
    // executable: base, dep-test, optparse-applicative (3)
    // test-suite: base, dep-test, tasty (3)
    assert_eq!(
        all_deps.len(),
        9,
        "expected 9 total deps, got {}",
        all_deps.len()
    );
}

// ============================================================================
// Dependency version ranges parsed correctly
// ============================================================================

#[test]
fn ast_dependency_version_ranges() {
    let source = "\
cabal-version: 3.0
name: vr-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends:
      base >=4.14 && <5
    , aeson ^>=2.2
    , text >=2.0
    , containers
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    let lib = ast.library.as_ref().expect("should have library");
    let deps = &lib.fields.build_depends;

    // base >=4.14 && <5
    let base_dep = deps.iter().find(|d| d.package == "base").expect("base dep");
    match &base_dep.version_range {
        Some(VersionRange::And(left, right)) => {
            match left.as_ref() {
                VersionRange::Gte(v) => assert_eq!(v.components, vec![4, 14]),
                other => panic!("expected Gte for base left, got: {other:?}"),
            }
            match right.as_ref() {
                VersionRange::Lt(v) => assert_eq!(v.components, vec![5]),
                other => panic!("expected Lt for base right, got: {other:?}"),
            }
        }
        other => panic!("expected And for base, got: {other:?}"),
    }

    // aeson ^>=2.2
    let aeson_dep = deps
        .iter()
        .find(|d| d.package == "aeson")
        .expect("aeson dep");
    match &aeson_dep.version_range {
        Some(VersionRange::MajorBound(v)) => assert_eq!(v.components, vec![2, 2]),
        other => panic!("expected MajorBound for aeson, got: {other:?}"),
    }

    // text >=2.0
    let text_dep = deps.iter().find(|d| d.package == "text").expect("text dep");
    match &text_dep.version_range {
        Some(VersionRange::Gte(v)) => assert_eq!(v.components, vec![2, 0]),
        other => panic!("expected Gte for text, got: {other:?}"),
    }

    // containers (no version)
    let cont_dep = deps
        .iter()
        .find(|d| d.package == "containers")
        .expect("containers dep");
    assert!(
        cont_dep.version_range.is_none(),
        "containers should have no version range"
    );
}

// ============================================================================
// all_components()
// ============================================================================

#[test]
fn ast_all_components_types() {
    let source = "\
cabal-version: 3.0
name: comp-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base
  default-language: GHC2021

executable comp-exe
  main-is: Main.hs
  build-depends: base
  default-language: GHC2021

test-suite comp-test-suite
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base
  default-language: GHC2021

benchmark comp-bench
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    let comps = ast.all_components();
    assert_eq!(comps.len(), 4, "expected 4 components");

    let kinds: Vec<&str> = comps
        .iter()
        .map(|c| match c {
            Component::Library(_) => "library",
            Component::Executable(_) => "executable",
            Component::TestSuite(_) => "test-suite",
            Component::Benchmark(_) => "benchmark",
        })
        .collect();
    assert!(kinds.contains(&"library"));
    assert!(kinds.contains(&"executable"));
    assert!(kinds.contains(&"test-suite"));
    assert!(kinds.contains(&"benchmark"));
}

// ============================================================================
// Common stanza imports tracked
// ============================================================================

#[test]
fn ast_common_stanza_imports() {
    let source = "\
cabal-version: 3.0
name: import-test
version: 0.1.0.0

common warnings
  ghc-options: -Wall

common lang
  default-language: GHC2021

library
  import: warnings
  import: lang
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    assert_eq!(ast.common_stanzas.len(), 2);
    assert_eq!(ast.common_stanzas[0].name, "warnings");
    assert_eq!(ast.common_stanzas[1].name, "lang");

    let lib = ast.library.as_ref().expect("should have library");
    assert_eq!(lib.fields.imports.len(), 2);
    assert!(lib.fields.imports.contains(&"warnings"));
    assert!(lib.fields.imports.contains(&"lang"));
}

// ============================================================================
// Conditional dependencies
// ============================================================================

#[test]
fn ast_conditional_dependencies() {
    let source = "\
cabal-version: 3.0
name: cond-dep-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  default-language: GHC2021
  if os(windows)
    build-depends: Win32 ^>=2.13
  else
    build-depends: unix ^>=2.7
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    let lib = ast.library.as_ref().expect("should have library");
    // Direct deps: just base.
    assert_eq!(lib.fields.build_depends.len(), 1);
    assert_eq!(lib.fields.build_depends[0].package, "base");

    // Should have conditionals.
    assert!(
        !lib.fields.conditionals.is_empty(),
        "should have conditionals"
    );
    let cond = &lib.fields.conditionals[0];

    // Then-branch should have Win32.
    assert_eq!(cond.then_deps.len(), 1);
    assert_eq!(cond.then_deps[0].package, "Win32");

    // Else-branch should have unix.
    assert_eq!(cond.else_deps.len(), 1);
    assert_eq!(cond.else_deps[0].package, "unix");
}

// ============================================================================
// Exposed modules
// ============================================================================

#[test]
fn ast_exposed_modules() {
    let source = "\
cabal-version: 3.0
name: mod-test
version: 0.1.0.0

library
  exposed-modules:
    Data.Foo
    Data.Bar
    Data.Baz.Internal
  build-depends: base >=4.14 && <5
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    let lib = ast.library.as_ref().expect("should have library");
    assert_eq!(lib.exposed_modules.len(), 3);
    assert!(lib.exposed_modules.contains(&"Data.Foo"));
    assert!(lib.exposed_modules.contains(&"Data.Bar"));
    assert!(lib.exposed_modules.contains(&"Data.Baz.Internal"));
}

// ============================================================================
// Default extensions
// ============================================================================

#[test]
fn ast_default_extensions() {
    let source = "\
cabal-version: 3.0
name: ext-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base >=4.14 && <5
  default-language: GHC2021
  default-extensions:
    OverloadedStrings
    DerivingStrategies
    LambdaCase
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    let lib = ast.library.as_ref().expect("should have library");
    assert_eq!(lib.fields.default_extensions.len(), 3);
    assert!(lib.fields.default_extensions.contains(&"OverloadedStrings"));
    assert!(lib
        .fields
        .default_extensions
        .contains(&"DerivingStrategies"));
    assert!(lib.fields.default_extensions.contains(&"LambdaCase"));
}

// ============================================================================
// Flags
// ============================================================================

#[test]
fn ast_flags() {
    let source = "\
cabal-version: 3.0
name: flag-test
version: 0.1.0.0

flag dev
  description: Development mode
  default: False
  manual: True

flag examples
  description: Build examples
  default: True

library
  exposed-modules: Lib
  build-depends: base
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    assert_eq!(ast.flags.len(), 2);
    let dev = &ast.flags[0];
    assert_eq!(dev.name, "dev");
    assert_eq!(dev.description, Some("Development mode"));
    assert_eq!(dev.default, Some(false));
    assert_eq!(dev.manual, Some(true));

    let examples = &ast.flags[1];
    assert_eq!(examples.name, "examples");
    assert_eq!(examples.default, Some(true));
}

// ============================================================================
// Source repositories
// ============================================================================

#[test]
fn ast_source_repositories() {
    let source = "\
cabal-version: 3.0
name: repo-test
version: 0.1.0.0

source-repository head
  type: git
  location: https://github.com/example/repo-test

library
  exposed-modules: Lib
  build-depends: base
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    assert_eq!(ast.source_repositories.len(), 1);
    let repo = &ast.source_repositories[0];
    assert_eq!(repo.kind, Some("head"));
    assert_eq!(repo.repo_type, Some("git"));
    assert_eq!(repo.location, Some("https://github.com/example/repo-test"));
}

// ============================================================================
// Executable fields
// ============================================================================

#[test]
fn ast_executable_fields() {
    let source = "\
cabal-version: 3.0
name: exe-test
version: 0.1.0.0

executable my-exe
  main-is: Main.hs
  other-modules: Paths_exe_test
  build-depends: base, exe-test
  hs-source-dirs: app
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    assert_eq!(ast.executables.len(), 1);
    let exe = &ast.executables[0];
    assert_eq!(exe.fields.name, Some("my-exe"));
    assert_eq!(exe.main_is, Some("Main.hs"));
    assert_eq!(exe.fields.hs_source_dirs, vec!["app"]);
    assert_eq!(exe.fields.default_language, Some("GHC2021"));
}

// ============================================================================
// Test suite fields
// ============================================================================

#[test]
fn ast_test_suite_fields() {
    let source = "\
cabal-version: 3.0
name: ts-test
version: 0.1.0.0

test-suite my-tests
  type: exitcode-stdio-1.0
  main-is: Main.hs
  build-depends: base, tasty ^>=1.5
  hs-source-dirs: test
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    assert_eq!(ast.test_suites.len(), 1);
    let ts = &ast.test_suites[0];
    assert_eq!(ts.fields.name, Some("my-tests"));
    assert_eq!(ts.test_type, Some("exitcode-stdio-1.0"));
    assert_eq!(ts.main_is, Some("Main.hs"));
}

// ============================================================================
// CST node back-references are valid
// ============================================================================

#[test]
fn ast_cst_node_references_valid() {
    let source = "\
cabal-version: 3.0
name: backref-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base >=4.14
  default-language: GHC2021
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    // Check root back-reference.
    assert_eq!(ast.cst_root.0, result.cst.root.0);

    // Check library back-reference is valid.
    let lib = ast.library.as_ref().expect("should have library");
    let lib_node = result.cst.node(lib.fields.cst_node);
    assert_eq!(
        lib_node.kind,
        cabalist_parser::CstNodeKind::Section,
        "library cst_node should point to a Section"
    );

    // Check dependency back-references are valid.
    for dep in &lib.fields.build_depends {
        let _node = result.cst.node(dep.cst_node); // should not panic
    }
}

// ============================================================================
// GHC options parsed
// ============================================================================

#[test]
fn ast_ghc_options() {
    let source = "\
cabal-version: 3.0
name: ghc-opt-test
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base
  default-language: GHC2021
  ghc-options: -Wall -Wcompat -Werror
";
    let result = parse(source);
    let ast = derive_ast(&result.cst);

    let lib = ast.library.as_ref().expect("should have library");
    assert!(lib.fields.ghc_options.contains(&"-Wall"));
    assert!(lib.fields.ghc_options.contains(&"-Wcompat"));
    assert!(lib.fields.ghc_options.contains(&"-Werror"));
}
