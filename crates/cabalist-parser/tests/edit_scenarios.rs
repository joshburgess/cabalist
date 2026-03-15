//! Edit scenario integration tests for the cabalist parser.
//!
//! Tests the full cycle: parse -> detect style -> apply edit -> verify formatting
//! -> re-parse -> verify clean -> reverse edit -> verify byte-identical.

use cabalist_parser::edit::{
    add_field_to_section, add_list_item, add_section, detect_list_style, find_field,
    remove_list_item, set_field_value, EditBatch, ListStyle, TextEdit,
};
use cabalist_parser::parse;

/// Helper: parse source, apply edits, return new source.
fn apply_edits(source: &str, edits: Vec<TextEdit>) -> String {
    let mut batch = EditBatch::new();
    batch.add_all(edits);
    batch.apply(source)
}

/// Helper: assert the given source parses cleanly (no diagnostics) and
/// round-trips.
fn assert_clean_parse(source: &str) {
    let result = parse(source);
    assert!(
        result.diagnostics.is_empty(),
        "expected no diagnostics after edit, got: {:?}\nsource:\n{}",
        result.diagnostics,
        source,
    );
    assert_eq!(
        result.cst.render(),
        source,
        "re-parse round-trip failed after edit"
    );
}

// ============================================================================
// NoComma style: add and remove modules
// ============================================================================

#[test]
fn edit_no_comma_add_then_remove_module() {
    let original = "\
library
  exposed-modules:
    Data.Map
    Data.Set
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "exposed-modules").unwrap();
    assert_eq!(detect_list_style(&result.cst, field), ListStyle::NoComma);

    // Add Data.List (sorted: between Map and Set).
    let edits = add_list_item(&result.cst, field, "Data.List", true);
    let added = apply_edits(original, edits);
    assert!(added.contains("Data.List"), "added item should be present");
    assert_clean_parse(&added);

    // Remove Data.List -> should restore original.
    let result2 = parse(&added);
    let section2 = result2.cst.node(result2.cst.root).children[0];
    let field2 = find_field(&result2.cst, section2, "exposed-modules").unwrap();
    let edits2 = remove_list_item(&result2.cst, field2, "Data.List");
    let removed = apply_edits(&added, edits2);
    assert_eq!(removed, original, "remove should restore original");
}

// ============================================================================
// LeadingComma style: add and remove dependency
// ============================================================================

#[test]
fn edit_leading_comma_add_then_remove_dep() {
    let original = "\
library
  build-depends:
      base >=4.14
    , text >=2.0
    , aeson ^>=2.2
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();
    assert_eq!(
        detect_list_style(&result.cst, field),
        ListStyle::LeadingComma
    );

    // Add containers (sorted: between base and text).
    let edits = add_list_item(&result.cst, field, "containers ^>=0.6", true);
    let added = apply_edits(original, edits);
    assert!(added.contains("containers"), "containers should be present");
    assert_clean_parse(&added);

    // Remove containers -> should restore original.
    let result2 = parse(&added);
    let section2 = result2.cst.node(result2.cst.root).children[0];
    let field2 = find_field(&result2.cst, section2, "build-depends").unwrap();
    let edits2 = remove_list_item(&result2.cst, field2, "containers");
    let removed = apply_edits(&added, edits2);
    assert_eq!(removed, original, "remove should restore original");
}

// ============================================================================
// TrailingComma style: add and remove dependency
// ============================================================================

#[test]
fn edit_trailing_comma_add_then_remove_dep() {
    let original = "\
library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();
    assert_eq!(
        detect_list_style(&result.cst, field),
        ListStyle::TrailingComma
    );

    // Add containers (sorted: between base and text).
    let edits = add_list_item(&result.cst, field, "containers ^>=0.6", true);
    let added = apply_edits(original, edits);
    assert!(added.contains("containers"), "containers should be present");
    assert_clean_parse(&added);

    // Remove containers -> should restore original.
    let result2 = parse(&added);
    let section2 = result2.cst.node(result2.cst.root).children[0];
    let field2 = find_field(&result2.cst, section2, "build-depends").unwrap();
    let edits2 = remove_list_item(&result2.cst, field2, "containers");
    let removed = apply_edits(&added, edits2);
    assert_eq!(removed, original, "remove should restore original");
}

// ============================================================================
// SingleLine style: add and remove dependency
// ============================================================================

#[test]
fn edit_single_line_add_then_remove_dep() {
    let original = "\
library
  build-depends: base >=4.14, text >=2.0, aeson ^>=2.2
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();
    assert_eq!(detect_list_style(&result.cst, field), ListStyle::SingleLine);

    // Add zlib (unsorted, appended to end for single-line).
    let edits = add_list_item(&result.cst, field, "zlib ^>=0.7", false);
    let added = apply_edits(original, edits);
    assert!(added.contains("zlib"), "zlib should be present");
    assert_clean_parse(&added);

    // Remove zlib -> should restore original.
    let result2 = parse(&added);
    let section2 = result2.cst.node(result2.cst.root).children[0];
    let field2 = find_field(&result2.cst, section2, "build-depends").unwrap();
    let edits2 = remove_list_item(&result2.cst, field2, "zlib");
    let removed = apply_edits(&added, edits2);
    assert_eq!(removed, original, "remove should restore original");
}

// ============================================================================
// Add dependency to empty build-depends field
// ============================================================================

#[test]
fn edit_add_to_empty_build_depends() {
    let original = "\
library
  build-depends:
  exposed-modules: Lib
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();

    let edits = add_list_item(&result.cst, field, "base >=4.14", false);
    let added = apply_edits(original, edits);
    assert!(added.contains("base >=4.14"), "base should be present");
    assert_clean_parse(&added);
}

// ============================================================================
// Add multiple deps in sequence (simulating a batch)
// ============================================================================

#[test]
fn edit_add_multiple_deps_sequentially() {
    let original = "\
library
  build-depends:
      base >=4.14
    , text >=2.0
";
    // Add first dep.
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();
    let edits = add_list_item(&result.cst, field, "aeson ^>=2.2", true);
    let after_first = apply_edits(original, edits);
    assert_clean_parse(&after_first);

    // Add second dep.
    let result2 = parse(&after_first);
    let section2 = result2.cst.node(result2.cst.root).children[0];
    let field2 = find_field(&result2.cst, section2, "build-depends").unwrap();
    let edits2 = add_list_item(&result2.cst, field2, "containers ^>=0.6", true);
    let after_second = apply_edits(&after_first, edits2);
    assert_clean_parse(&after_second);

    // Verify all three deps are present in sorted order.
    let aeson_pos = after_second.find("aeson").unwrap();
    let base_pos = after_second.find("base").unwrap();
    let containers_pos = after_second.find("containers").unwrap();
    let text_pos = after_second.find("text").unwrap();
    assert!(aeson_pos < base_pos);
    assert!(base_pos < containers_pos);
    assert!(containers_pos < text_pos);
}

// ============================================================================
// Add module to exposed-modules (NoComma)
// ============================================================================

#[test]
fn edit_add_module_to_exposed_modules() {
    let original = "\
library
  exposed-modules:
    MyLib
    MyLib.Types
  build-depends: base >=4.14 && <5
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "exposed-modules").unwrap();

    let edits = add_list_item(&result.cst, field, "MyLib.Internal", true);
    let added = apply_edits(original, edits);
    assert!(added.contains("MyLib.Internal"));
    assert_clean_parse(&added);

    // Should be sorted between MyLib and MyLib.Types.
    let internal_pos = added.find("MyLib.Internal").unwrap();
    let types_pos = added.find("MyLib.Types").unwrap();
    assert!(
        internal_pos < types_pos,
        "MyLib.Internal should sort before MyLib.Types"
    );
}

// ============================================================================
// Remove a module
// ============================================================================

#[test]
fn edit_remove_module() {
    let original = "\
library
  exposed-modules:
    MyLib
    MyLib.Internal
    MyLib.Types
  build-depends: base
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "exposed-modules").unwrap();

    let edits = remove_list_item(&result.cst, field, "MyLib.Internal");
    let removed = apply_edits(original, edits);
    assert!(!removed.contains("MyLib.Internal"));
    assert!(removed.contains("MyLib"));
    assert!(removed.contains("MyLib.Types"));
    assert_clean_parse(&removed);
}

// ============================================================================
// Set scalar field value (change version)
// ============================================================================

#[test]
fn edit_set_version() {
    let original = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0
";
    let result = parse(original);
    let field = find_field(&result.cst, result.cst.root, "version").unwrap();

    let edit = set_field_value(&result.cst, field, "1.0.0.0");
    let edited = apply_edits(original, vec![edit]);
    assert!(edited.contains("version: 1.0.0.0"));
    assert!(!edited.contains("0.1.0.0"));
    assert_clean_parse(&edited);
}

// ============================================================================
// Set field value for name
// ============================================================================

#[test]
fn edit_set_name() {
    let original = "\
cabal-version: 3.0
name: old-name
version: 0.1.0.0
";
    let result = parse(original);
    let field = find_field(&result.cst, result.cst.root, "name").unwrap();

    let edit = set_field_value(&result.cst, field, "new-name");
    let edited = apply_edits(original, vec![edit]);
    assert!(edited.contains("name: new-name"));
    assert!(!edited.contains("old-name"));
    assert_clean_parse(&edited);
}

// ============================================================================
// Add a field to a section
// ============================================================================

#[test]
fn edit_add_field_to_section() {
    let original = "\
library
  exposed-modules: Lib
  build-depends: base
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];

    let edit = add_field_to_section(&result.cst, section, "default-language", "GHC2021");
    let edited = apply_edits(original, vec![edit]);
    assert!(edited.contains("default-language: GHC2021"));
    assert_clean_parse(&edited);
}

// ============================================================================
// Add a new section to the file
// ============================================================================

#[test]
fn edit_add_new_library_section() {
    let original = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0
";
    let result = parse(original);
    let edit = add_section(
        &result.cst,
        "library",
        None,
        &[
            ("exposed-modules", "Lib"),
            ("build-depends", "base >=4.14 && <5"),
            ("hs-source-dirs", "src"),
            ("default-language", "GHC2021"),
        ],
        2,
    );
    let edited = apply_edits(original, vec![edit]);
    assert!(edited.contains("library\n"));
    assert!(edited.contains("  exposed-modules: Lib\n"));
    assert!(edited.contains("  build-depends: base >=4.14 && <5\n"));
    assert!(edited.contains("  hs-source-dirs: src\n"));
    assert!(edited.contains("  default-language: GHC2021\n"));
    assert_clean_parse(&edited);
}

// ============================================================================
// Add a named executable section
// ============================================================================

#[test]
fn edit_add_new_executable_section() {
    let original = "\
cabal-version: 3.0
name: my-pkg
version: 0.1.0.0

library
  exposed-modules: Lib
  build-depends: base
";
    let result = parse(original);
    let edit = add_section(
        &result.cst,
        "executable",
        Some("my-exe"),
        &[
            ("main-is", "Main.hs"),
            ("build-depends", "base, my-pkg"),
            ("hs-source-dirs", "app"),
        ],
        2,
    );
    let edited = apply_edits(original, vec![edit]);
    assert!(edited.contains("executable my-exe\n"));
    assert!(edited.contains("  main-is: Main.hs\n"));
    assert_clean_parse(&edited);
}

// ============================================================================
// Remove first item from trailing-comma list
// ============================================================================

#[test]
fn edit_remove_first_trailing_comma() {
    let original = "\
library
  build-depends:
    base >=4.14,
    text >=2.0,
    aeson ^>=2.2
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();

    let edits = remove_list_item(&result.cst, field, "base");
    let removed = apply_edits(original, edits);
    assert!(!removed.contains("base"));
    assert!(removed.contains("text"));
    assert!(removed.contains("aeson"));
    assert_clean_parse(&removed);
}

// ============================================================================
// Remove last item from leading-comma list
// ============================================================================

#[test]
fn edit_remove_last_leading_comma() {
    let original = "\
library
  build-depends:
      base >=4.14
    , text >=2.0
    , aeson ^>=2.2
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();

    let edits = remove_list_item(&result.cst, field, "aeson");
    let removed = apply_edits(original, edits);
    assert!(!removed.contains("aeson"));
    assert!(removed.contains("base"));
    assert!(removed.contains("text"));
    assert_clean_parse(&removed);
}

// ============================================================================
// Remove from single-line list (first item)
// ============================================================================

#[test]
fn edit_remove_first_single_line() {
    let original = "\
library
  build-depends: base >=4.14, text >=2.0, aeson ^>=2.2
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();

    let edits = remove_list_item(&result.cst, field, "base");
    let removed = apply_edits(original, edits);
    assert!(!removed.contains("base"));
    assert!(removed.contains("text >=2.0"));
    assert!(removed.contains("aeson ^>=2.2"));
    assert_clean_parse(&removed);
}

// ============================================================================
// Add dep at beginning (sorted, alphabetically before first)
// ============================================================================

#[test]
fn edit_add_dep_at_beginning_leading_comma() {
    let original = "\
library
  build-depends:
      containers ^>=0.6
    , text >=2.0
";
    let result = parse(original);
    let section = result.cst.node(result.cst.root).children[0];
    let field = find_field(&result.cst, section, "build-depends").unwrap();

    let edits = add_list_item(&result.cst, field, "base >=4.14", true);
    let added = apply_edits(original, edits);
    assert!(added.contains("base >=4.14"));
    assert_clean_parse(&added);

    // base should come before containers.
    let base_pos = added.find("base").unwrap();
    let cont_pos = added.find("containers").unwrap();
    assert!(base_pos < cont_pos);
}
