//! Edit engine stress test — runs add/remove operations against all 100
//! real-world .cabal fixtures to catch edge cases.
//!
//! For each fixture:
//! 1. Parse the file
//! 2. Find the first section with `build-depends`
//! 3. Add a fake dependency "zzz-test-package"
//! 4. Verify the edited source re-parses cleanly
//! 5. Remove "zzz-test-package"
//! 6. Verify byte-identical to original
//!
//! Also tests module add/remove on `exposed-modules` and scalar field
//! set/restore on `version`.

use cabalist_parser::edit::{
    add_list_item, find_field, remove_list_item, set_field_value, EditBatch, TextEdit,
};
use cabalist_parser::{parse, CstNodeKind, Severity};
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/cabalist-parser -> crates
    path.pop(); // crates -> workspace root
    path.push("tests");
    path.push("fixtures");
    path.push("real-world");
    path
}

fn apply_edits(source: &str, edits: Vec<TextEdit>) -> String {
    let mut batch = EditBatch::new();
    batch.add_all(edits);
    batch.apply(source)
}

/// Collect all .cabal fixture filenames, sorted.
fn all_fixture_files() -> Vec<String> {
    let dir = fixtures_dir();
    assert!(
        dir.exists(),
        "Fixtures directory not found: {}",
        dir.display()
    );

    let mut files: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "cabal")
                .unwrap_or(false)
        })
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    files.sort();
    assert!(!files.is_empty(), "No .cabal fixture files found");
    files
}

/// Find the first section that contains a `build-depends` field.
/// Returns `(section_id, field_id)` or `None` if no section has one.
fn find_first_build_depends(
    cst: &cabalist_parser::CabalCst,
) -> Option<(cabalist_parser::NodeId, cabalist_parser::NodeId)> {
    let root = cst.node(cst.root);
    for &child_id in &root.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Section {
            if let Some(field_id) = find_field(cst, child_id, "build-depends") {
                return Some((child_id, field_id));
            }
        }
    }
    None
}

/// Find the first section that contains an `exposed-modules` field.
fn find_first_exposed_modules(
    cst: &cabalist_parser::CabalCst,
) -> Option<(cabalist_parser::NodeId, cabalist_parser::NodeId)> {
    let root = cst.node(cst.root);
    for &child_id in &root.children {
        let child = cst.node(child_id);
        if child.kind == CstNodeKind::Section {
            if let Some(field_id) = find_field(cst, child_id, "exposed-modules") {
                return Some((child_id, field_id));
            }
        }
    }
    None
}

// ==========================================================================
// Test 1: Add/remove dependency round-trip across all fixtures
// ==========================================================================

#[test]
fn add_remove_dep_round_trip_all_fixtures() {
    let dir = fixtures_dir();
    let files = all_fixture_files();

    let mut tested = 0usize;
    let mut skipped = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for filename in &files {
        let path = dir.join(filename);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

        let result = parse(&source);
        let cst = &result.cst;

        // Find first section with build-depends.
        let (section_id, field_id) = match find_first_build_depends(cst) {
            Some(ids) => ids,
            None => {
                skipped += 1;
                eprintln!("  SKIP {filename}: no section with build-depends");
                continue;
            }
        };
        let _ = section_id; // used only for finding, field_id is what we edit

        // Step 1: Add "zzz-test-package ^>=99.0"
        let edits = add_list_item(cst, field_id, "zzz-test-package ^>=99.0", true);
        if edits.is_empty() {
            failures.push(format!("{filename}: add_list_item returned no edits"));
            continue;
        }
        let added_source = apply_edits(&source, edits);

        // Step 2: Verify the added source contains our dep
        if !added_source.contains("zzz-test-package") {
            failures.push(format!(
                "{filename}: added source does not contain 'zzz-test-package'"
            ));
            continue;
        }

        // Step 3: Re-parse and verify no parse errors
        let result2 = parse(&added_source);
        let errors: Vec<_> = result2
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        if !errors.is_empty() {
            failures.push(format!("{filename}: parse errors after add: {:?}", errors));
            continue;
        }

        // Verify round-trip of the edited source
        let rendered2 = result2.cst.render();
        if rendered2 != added_source {
            failures.push(format!(
                "{filename}: re-parse round-trip failed after add (len {} vs {})",
                added_source.len(),
                rendered2.len()
            ));
            continue;
        }

        // Step 4: Find build-depends again in the re-parsed CST
        let field_id2 = match find_first_build_depends(&result2.cst) {
            Some((_, fid)) => fid,
            None => {
                failures.push(format!("{filename}: build-depends not found after add"));
                continue;
            }
        };

        // Step 5: Remove "zzz-test-package"
        let edits2 = remove_list_item(&result2.cst, field_id2, "zzz-test-package");
        if edits2.is_empty() {
            failures.push(format!("{filename}: remove_list_item returned no edits"));
            continue;
        }
        let removed_source = apply_edits(&added_source, edits2);

        // Step 6: Verify byte-identical to original
        if removed_source != source {
            let first_diff = source
                .as_bytes()
                .iter()
                .zip(removed_source.as_bytes().iter())
                .position(|(a, b)| a != b)
                .unwrap_or(source.len().min(removed_source.len()));
            let line = source[..first_diff.min(source.len())].matches('\n').count() + 1;
            failures.push(format!(
                "{filename}: add+remove not byte-identical (diff at line {line}, byte {first_diff}, \
                 orig={} bytes, result={} bytes)",
                source.len(),
                removed_source.len()
            ));
            continue;
        }

        tested += 1;
    }

    eprintln!(
        "\nadd_remove_dep: {tested} tested, {skipped} skipped, {} failures out of {} total",
        failures.len(),
        files.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} fixture(s) failed add/remove dep round-trip:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }

    assert!(tested > 0, "No fixtures were tested (all skipped)");
}

// ==========================================================================
// Test 2: Add/remove module round-trip on exposed-modules
// ==========================================================================

#[test]
fn add_remove_module_round_trip_all_fixtures() {
    let dir = fixtures_dir();
    let files = all_fixture_files();

    let mut tested = 0usize;
    let mut skipped = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for filename in &files {
        let path = dir.join(filename);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

        let result = parse(&source);
        let cst = &result.cst;

        // Find first section with exposed-modules.
        let (_section_id, field_id) = match find_first_exposed_modules(cst) {
            Some(ids) => ids,
            None => {
                skipped += 1;
                continue;
            }
        };

        // Add a fake module
        let edits = add_list_item(cst, field_id, "Zzz.Test.Module", true);
        if edits.is_empty() {
            failures.push(format!(
                "{filename}: add_list_item (module) returned no edits"
            ));
            continue;
        }
        let added_source = apply_edits(&source, edits);

        if !added_source.contains("Zzz.Test.Module") {
            failures.push(format!(
                "{filename}: added source does not contain 'Zzz.Test.Module'"
            ));
            continue;
        }

        // Re-parse
        let result2 = parse(&added_source);
        let errors: Vec<_> = result2
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        if !errors.is_empty() {
            failures.push(format!(
                "{filename}: parse errors after module add: {:?}",
                errors
            ));
            continue;
        }

        let rendered2 = result2.cst.render();
        if rendered2 != added_source {
            failures.push(format!(
                "{filename}: re-parse round-trip failed after module add"
            ));
            continue;
        }

        // Remove the module
        let field_id2 = match find_first_exposed_modules(&result2.cst) {
            Some((_, fid)) => fid,
            None => {
                failures.push(format!("{filename}: exposed-modules not found after add"));
                continue;
            }
        };

        let edits2 = remove_list_item(&result2.cst, field_id2, "Zzz.Test.Module");
        if edits2.is_empty() {
            failures.push(format!(
                "{filename}: remove_list_item (module) returned no edits"
            ));
            continue;
        }
        let removed_source = apply_edits(&added_source, edits2);

        if removed_source != source {
            let first_diff = source
                .as_bytes()
                .iter()
                .zip(removed_source.as_bytes().iter())
                .position(|(a, b)| a != b)
                .unwrap_or(source.len().min(removed_source.len()));
            let line = source[..first_diff.min(source.len())].matches('\n').count() + 1;
            failures.push(format!(
                "{filename}: module add+remove not byte-identical (diff at line {line}, byte {first_diff})"
            ));
            continue;
        }

        tested += 1;
    }

    eprintln!(
        "\nadd_remove_module: {tested} tested, {skipped} skipped, {} failures out of {} total",
        failures.len(),
        files.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} fixture(s) failed add/remove module round-trip:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }

    assert!(tested > 0, "No fixtures with exposed-modules were tested");
}

// ==========================================================================
// Test 3: Set and restore scalar field value (version)
// ==========================================================================

#[test]
fn set_restore_version_all_fixtures() {
    let dir = fixtures_dir();
    let files = all_fixture_files();

    let mut tested = 0usize;
    let mut skipped = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for filename in &files {
        let path = dir.join(filename);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

        let result = parse(&source);
        let cst = &result.cst;

        // Find the top-level version field.
        let field_id = match find_field(cst, cst.root, "version") {
            Some(fid) => fid,
            None => {
                skipped += 1;
                continue;
            }
        };

        // Read the current value.
        let node = cst.node(field_id);
        let original_value = match node.field_value {
            Some(fv) => fv.slice(&cst.source).to_string(),
            None => {
                skipped += 1;
                continue;
            }
        };

        // Set to a fake version.
        let edit = set_field_value(cst, field_id, "99.99.99.99");
        let edited_source = apply_edits(&source, vec![edit]);

        if !edited_source.contains("99.99.99.99") {
            failures.push(format!(
                "{filename}: edited source does not contain '99.99.99.99'"
            ));
            continue;
        }

        // Re-parse
        let result2 = parse(&edited_source);
        let rendered2 = result2.cst.render();
        if rendered2 != edited_source {
            failures.push(format!(
                "{filename}: re-parse round-trip failed after version set"
            ));
            continue;
        }

        // Restore original value.
        let field_id2 = match find_field(&result2.cst, result2.cst.root, "version") {
            Some(fid) => fid,
            None => {
                failures.push(format!("{filename}: version field not found after set"));
                continue;
            }
        };

        let edit2 = set_field_value(&result2.cst, field_id2, original_value.trim());
        let restored_source = apply_edits(&edited_source, vec![edit2]);

        if restored_source != source {
            let first_diff = source
                .as_bytes()
                .iter()
                .zip(restored_source.as_bytes().iter())
                .position(|(a, b)| a != b)
                .unwrap_or(source.len().min(restored_source.len()));
            failures.push(format!(
                "{filename}: version set+restore not byte-identical (diff at byte {first_diff}, \
                 original value={:?})",
                original_value.trim()
            ));
            continue;
        }

        tested += 1;
    }

    eprintln!(
        "\nset_restore_version: {tested} tested, {skipped} skipped, {} failures out of {} total",
        failures.len(),
        files.len()
    );

    if !failures.is_empty() {
        panic!(
            "{} fixture(s) failed version set+restore:\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }

    assert!(tested > 0, "No fixtures with version field were tested");
}
