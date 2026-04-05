//! Real-world regression tests.
//!
//! Runs various operations against the full corpus of 100+ real-world .cabal
//! files to catch regressions that synthetic test cases miss.

use cabalist_opinions::lints::{run_lints_with_cst, LintConfig};
use cabalist_parser::ast::derive_ast;
use cabalist_parser::diagnostic::Severity;
use cabalist_parser::edit::{
    self, add_list_item, find_field, find_section, remove_list_item, EditBatch,
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

// ============================================================================
// Conditional deps: deps inside `if impl(ghc ...)` blocks must be extracted
// ============================================================================

#[test]
fn conditional_deps_extracted_from_real_world_files() {
    let mut failures = Vec::new();

    for (name, source) in all_fixture_sources() {
        // Only test files that actually have conditional blocks.
        if !source.contains("if ") {
            continue;
        }

        let result = parse(&source);
        let ast = derive_ast(&result.cst);

        // Collect deps from top-level build-depends only.
        let top_level_deps: usize = ast
            .all_components()
            .iter()
            .map(|c| c.fields().build_depends.len())
            .sum();

        // Collect ALL deps including conditionals via all_dependencies.
        let all_deps = ast.all_dependencies();

        // If the file has conditionals, all_dependencies should find at least
        // as many deps as the top-level count. If there are conditional deps,
        // it should find MORE.
        if all_deps.len() < top_level_deps {
            failures.push(format!(
                "{name}: all_dependencies ({}) < top-level deps ({top_level_deps})",
                all_deps.len()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Conditional dep extraction failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn files_with_conditionals_have_conditional_deps() {
    // Specific files known to have deps inside conditionals.
    let known_conditional_dep_files = [
        "aeson.cabal",     // `if !impl(ghc >=9.0)` → integer-gmp
        "lens.cabal",      // various `if flag(...)` deps
        "text.cabal",      // `if impl(ghc >=...)` deps
    ];

    let fixtures = all_fixture_sources();

    for target in &known_conditional_dep_files {
        let Some((name, source)) = fixtures.iter().find(|(n, _)| n == target) else {
            continue;
        };

        let result = parse(&source);
        let ast = derive_ast(&result.cst);

        // Check that at least one component has non-empty conditionals.
        let has_conditional = ast.all_components().iter().any(|c| {
            !c.fields().conditionals.is_empty()
        });

        assert!(
            has_conditional,
            "{name}: should have conditional blocks in at least one component"
        );

        // Check that conditional deps are found.
        let all_deps = ast.all_dependencies();
        let top_deps: usize = ast
            .all_components()
            .iter()
            .map(|c| c.fields().build_depends.len())
            .sum();

        assert!(
            all_deps.len() > top_deps,
            "{name}: all_dependencies ({}) should exceed top-level deps ({top_deps}) \
             because this file has deps inside conditionals",
            all_deps.len()
        );
    }
}

// ============================================================================
// Common stanza resolution: imports field is populated correctly
// ============================================================================

#[test]
fn common_stanza_imports_parsed_from_real_world_files() {
    let mut files_with_stanzas = 0;
    let mut failures = Vec::new();

    for (name, source) in all_fixture_sources() {
        let result = parse(&source);
        let ast = derive_ast(&result.cst);

        if ast.common_stanzas.is_empty() {
            continue;
        }
        files_with_stanzas += 1;

        let stanza_names: Vec<&str> = ast
            .common_stanzas
            .iter()
            .map(|s| s.fields.name.unwrap_or("(unnamed)"))
            .collect();

        // Every component that uses `import:` should reference a stanza
        // that actually exists.
        for comp in ast.all_components() {
            for import in &comp.fields().imports {
                if !stanza_names.iter().any(|s| s == import) {
                    failures.push(format!(
                        "{name}: component imports '{import}' but available stanzas are: {stanza_names:?}"
                    ));
                }
            }
        }
    }

    assert!(
        files_with_stanzas > 5,
        "expected at least 5 files with common stanzas, found {files_with_stanzas}"
    );
    assert!(
        failures.is_empty(),
        "Common stanza import mismatches:\n{}",
        failures.join("\n")
    );
}

// ============================================================================
// Set field round-trip: set version then set it back, file is unchanged
// ============================================================================

#[test]
fn set_field_round_trip_on_real_world_files() {
    let mut tested = 0;
    let mut failures = Vec::new();

    for (name, source) in all_fixture_sources() {
        let result = parse(&source);
        let cst = &result.cst;

        // Find the version field.
        let Some(version_field) = edit::find_field(cst, cst.root, "version") else {
            continue;
        };

        // Read original version.
        let original_version = {
            let node = cst.node(version_field);
            match node.field_value.as_ref() {
                Some(span) => span.slice(&cst.source).trim().to_string(),
                None => continue,
            }
        };

        if original_version.is_empty() {
            continue;
        }

        // Set to a different version.
        let edit1 = edit::set_field_value(cst, version_field, "99.99.99.99");
        let mut batch1 = EditBatch::new();
        batch1.add_all(vec![edit1]);
        let after_set = batch1.apply(&cst.source);

        // Verify the new version is present.
        if !after_set.contains("99.99.99.99") {
            failures.push(format!("{name}: set did not apply"));
            continue;
        }

        // Set back to original.
        let result2 = parse(&after_set);
        let Some(version_field2) = edit::find_field(&result2.cst, result2.cst.root, "version")
        else {
            failures.push(format!("{name}: version field disappeared after set"));
            continue;
        };

        let edit2 = edit::set_field_value(&result2.cst, version_field2, &original_version);
        let mut batch2 = EditBatch::new();
        batch2.add_all(vec![edit2]);
        let after_restore = batch2.apply(&result2.cst.source);

        if source != after_restore {
            failures.push(format!("{name}: set+restore did not produce original"));
        }

        tested += 1;
    }

    assert!(
        tested > 50,
        "should test at least 50 files, only tested {tested}"
    );
    assert!(
        failures.is_empty(),
        "Set field round-trip failures ({}/{tested}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
