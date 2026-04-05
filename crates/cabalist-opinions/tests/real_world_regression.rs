//! Real-world regression tests.
//!
//! Runs various operations against the full corpus of 100+ real-world .cabal
//! files to catch regressions that synthetic test cases miss.

use cabalist_opinions::lints::{run_lints_with_cst, LintConfig};
use cabalist_parser::ast::derive_ast;
use cabalist_parser::diagnostic::Severity;
use cabalist_parser::edit::{
    add_list_item, find_field, find_section, remove_list_item, EditBatch,
};
use cabalist_parser::parse;
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("tests");
    path.push("fixtures");
    path.push("real-world");
    path
}

fn all_fixture_sources() -> Vec<(String, String)> {
    let dir = fixtures_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(true, |e| e != "cabal") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let source = fs::read_to_string(&path).unwrap();
        files.push((name, source));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

// ============================================================================
// Formatter idempotency: parse(render(parse(x))) == render(parse(x))
//
// Note: sort_list_field may change multiline layout to single-line on first
// pass, which is intentional. We test that parse→render is idempotent, not
// that sorting is layout-preserving.
// ============================================================================

#[test]
fn parse_render_idempotent_on_real_world_files() {
    for (name, source) in all_fixture_sources() {
        // First pass: parse and render.
        let result1 = parse(&source);
        let rendered1 = result1.cst.render();

        // Second pass: parse the rendered output and render again.
        let result2 = parse(&rendered1);
        let rendered2 = result2.cst.render();

        assert_eq!(
            rendered1, rendered2,
            "{name}: parse→render is not idempotent"
        );
    }
}

// ============================================================================
// No validation errors on valid files (warnings are expected)
// ============================================================================

#[test]
fn no_unexpected_validation_errors_on_real_world_files() {
    let mut failures = Vec::new();

    for (name, source) in all_fixture_sources() {
        let result = parse(&source);
        let diags = cabalist_parser::validate(&result.cst);

        let unexpected: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .filter(|d| {
                // Known acceptable errors in some fixtures.
                !d.message.contains("missing required field")
                    && !d.message.contains("`cabal-version` should be the first")
                    && !d.message.contains("duplicate section")
            })
            .collect();

        if !unexpected.is_empty() {
            failures.push(format!(
                "{name}: {} unexpected error(s): {}",
                unexpected.len(),
                unexpected
                    .iter()
                    .map(|d| d.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Validation errors in real-world files:\n{}",
        failures.join("\n")
    );
}

// ============================================================================
// AST completeness: every file has a name and at least one component
// ============================================================================

#[test]
fn ast_extracts_components_from_real_world_files() {
    let mut failures = Vec::new();

    for (name, source) in all_fixture_sources() {
        let result = parse(&source);
        let ast = derive_ast(&result.cst);

        let components = ast.all_components();
        if components.is_empty() {
            failures.push(format!("{name}: no components extracted"));
        }
    }

    assert!(
        failures.is_empty(),
        "AST extraction failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn ast_extracts_dependencies_from_real_world_files() {
    let mut no_deps = Vec::new();

    for (name, source) in all_fixture_sources() {
        let result = parse(&source);
        let ast = derive_ast(&result.cst);

        let total_deps: usize = ast
            .all_components()
            .iter()
            .map(|c| c.fields().build_depends.len())
            .sum();

        if total_deps == 0 {
            no_deps.push(name);
        }
    }

    // Allow a small number of fixture files to have zero parsed deps
    // (e.g., files where deps are entirely inside conditionals that the
    // AST doesn't flatten).
    assert!(
        no_deps.len() <= 5,
        "Too many files with zero dependencies ({}/{}): {:?}",
        no_deps.len(),
        all_fixture_sources().len(),
        no_deps
    );
}

// ============================================================================
// Edit round-trip: add a dep then remove it, file is unchanged
// ============================================================================

#[test]
fn add_remove_dep_round_trip_on_real_world_files() {
    let mut tested = 0;
    let mut failures = Vec::new();

    for (name, source) in all_fixture_sources() {
        let result = parse(&source);

        // Find the library section.
        let Some(section) = find_section(&result.cst, "library", None) else {
            continue;
        };
        let Some(bd_field) = find_field(&result.cst, section, "build-depends") else {
            continue;
        };

        // Add a fake dependency.
        let add_edits =
            add_list_item(&result.cst, bd_field, "zzz-fake-test-dep ^>=99.0", false);
        if add_edits.is_empty() {
            continue;
        }
        let mut batch = EditBatch::new();
        batch.add_all(add_edits);
        let after_add = batch.apply(&source);

        if !after_add.contains("zzz-fake-test-dep") {
            failures.push(format!("{name}: added dep not found in output"));
            continue;
        }

        // Re-parse and remove.
        let result2 = parse(&after_add);
        let Some(section2) = find_section(&result2.cst, "library", None) else {
            failures.push(format!("{name}: library section disappeared after add"));
            continue;
        };
        let Some(bd_field2) = find_field(&result2.cst, section2, "build-depends") else {
            failures.push(format!("{name}: build-depends disappeared after add"));
            continue;
        };

        let remove_edits = remove_list_item(&result2.cst, bd_field2, "zzz-fake-test-dep");
        if remove_edits.is_empty() {
            continue; // Some styles don't support clean removal.
        }
        let mut batch2 = EditBatch::new();
        batch2.add_all(remove_edits);
        let after_remove = batch2.apply(&after_add);

        if source != after_remove {
            failures.push(format!("{name}: add+remove did not restore original"));
        }

        tested += 1;
    }

    assert!(tested > 20, "should test at least 20 files, only tested {tested}");
    assert!(
        failures.is_empty(),
        "Edit round-trip failures ({}/{tested}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ============================================================================
// Lint counts are reasonable
// ============================================================================

#[test]
fn no_error_severity_lints_on_real_world_files() {
    let mut failures = Vec::new();

    for (name, source) in all_fixture_sources() {
        let result = parse(&source);
        let ast = derive_ast(&result.cst);
        let lints = run_lints_with_cst(&ast, Some(&result.cst), &LintConfig::default());

        let errors: Vec<_> = lints
            .iter()
            .filter(|l| l.severity == Severity::Error)
            .collect();

        if !errors.is_empty() {
            failures.push(format!(
                "{name}: {} error-level lints: {}",
                errors.len(),
                errors
                    .iter()
                    .map(|l| format!("[{}] {}", l.id, l.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Error-level lints on real-world files:\n{}",
        failures.join("\n")
    );
}
