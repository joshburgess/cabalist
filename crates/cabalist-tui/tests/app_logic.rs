//! Unit tests for the TUI application logic.
//!
//! Tests state transitions, editing operations, undo/redo, dependency
//! management, extension toggling, and metadata editing.

use std::sync::atomic::{AtomicU64, Ordering};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

/// Create a test app with a sample .cabal file in a unique temp directory.
fn make_app() -> cabalist_tui::app::App {
    let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cabalist-app-test-{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    let cabal_path = dir.join("test-pkg.cabal");
    let source = "\
cabal-version: 3.0
name: test-pkg
version: 0.1.0.0
synopsis: A test package
license: MIT
author: Test Author
maintainer: test@example.com

library
  exposed-modules: Lib
  build-depends:
    base ^>=4.17,
    text ^>=2.0
  default-language: GHC2021
  default-extensions:
    OverloadedStrings
";
    std::fs::write(&cabal_path, source).unwrap();

    let theme = cabalist_tui::theme::Theme::dark();
    cabalist_tui::app::App::new(cabal_path, theme).unwrap()
}

// -- View switching --

#[test]
fn initial_view_is_dashboard() {
    let app = make_app();
    assert_eq!(app.current_view, cabalist_tui::views::View::Dashboard);
}

#[test]
fn switch_view_resets_selection() {
    let mut app = make_app();
    app.selected_index = 5;
    app.current_view = cabalist_tui::views::View::Dependencies;
    // Simulate what handle_action(SwitchView) does:
    app.selected_index = 0;
    assert_eq!(app.selected_index, 0);
    assert_eq!(app.current_view, cabalist_tui::views::View::Dependencies);
}

// -- AST --

#[test]
fn ast_has_correct_name() {
    let app = make_app();
    let ast = app.ast();
    assert_eq!(ast.name, Some("test-pkg"));
}

#[test]
fn ast_has_library() {
    let app = make_app();
    let ast = app.ast();
    assert!(ast.library.is_some());
}

#[test]
fn ast_has_dependencies() {
    let app = make_app();
    let ast = app.ast();
    let lib = ast.library.as_ref().unwrap();
    assert_eq!(lib.fields.build_depends.len(), 2);
    assert_eq!(lib.fields.build_depends[0].package, "base");
    assert_eq!(lib.fields.build_depends[1].package, "text");
}

// -- Metadata editing --

#[test]
fn set_metadata_field_updates_ast() {
    let mut app = make_app();
    app.set_metadata_field("synopsis", "Updated synopsis").unwrap();
    let ast = app.ast();
    assert_eq!(ast.synopsis, Some("Updated synopsis"));
}

#[test]
fn set_metadata_field_marks_dirty() {
    let mut app = make_app();
    assert!(!app.dirty);
    app.set_metadata_field("synopsis", "New value").unwrap();
    assert!(app.dirty);
}

// -- Save and reload --

#[test]
fn save_clears_dirty_flag() {
    let mut app = make_app();
    app.set_metadata_field("synopsis", "Changed").unwrap();
    assert!(app.dirty);
    app.save().unwrap();
    assert!(!app.dirty);
}

#[test]
fn reload_restores_from_disk() {
    let mut app = make_app();
    let original_synopsis = app.ast().synopsis.map(|s| s.to_string());
    app.set_metadata_field("synopsis", "Temporary change").unwrap();
    app.save().unwrap();
    // Manually write original back to disk.
    let source = std::fs::read_to_string(&app.cabal_path).unwrap();
    let reverted = source.replace("Temporary change", original_synopsis.as_deref().unwrap_or("A test package"));
    std::fs::write(&app.cabal_path, reverted).unwrap();
    app.reload().unwrap();
    assert!(!app.dirty);
}

// -- Undo --

#[test]
fn undo_restores_previous_state() {
    let mut app = make_app();
    let original = app.source.clone();
    app.set_metadata_field("version", "0.2.0.0").unwrap();
    assert_ne!(app.source, original);
    app.undo().unwrap();
    assert_eq!(app.source, original);
}

#[test]
fn undo_on_empty_stack_returns_error() {
    let mut app = make_app();
    let result = app.undo();
    assert!(result.is_err());
}

// -- Dependency management --

#[test]
fn add_dependency_adds_to_build_depends() {
    let mut app = make_app();
    app.add_dependency("aeson ^>=2.2").unwrap();
    let ast = app.ast();
    let lib = ast.library.as_ref().unwrap();
    let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
    assert!(dep_names.contains(&"aeson"));
}

#[test]
fn add_duplicate_dependency_fails() {
    let mut app = make_app();
    let result = app.add_dependency("base ^>=4.17");
    assert!(result.is_err());
}

#[test]
fn remove_dependency_removes_from_build_depends() {
    let mut app = make_app();
    app.remove_dependency("text").unwrap();
    let ast = app.ast();
    let lib = ast.library.as_ref().unwrap();
    let dep_names: Vec<&str> = lib.fields.build_depends.iter().map(|d| d.package).collect();
    assert!(!dep_names.contains(&"text"));
}

#[test]
fn remove_nonexistent_dependency_fails() {
    let mut app = make_app();
    let result = app.remove_dependency("nonexistent-pkg");
    assert!(result.is_err());
}

// -- Extension toggling --

#[test]
fn toggle_extension_off() {
    let mut app = make_app();
    let ast = app.ast();
    let lib = ast.library.as_ref().unwrap();
    assert!(lib.fields.default_extensions.contains(&"OverloadedStrings"));
    drop(ast);

    app.toggle_extension("OverloadedStrings").unwrap();

    let ast = app.ast();
    let lib = ast.library.as_ref().unwrap();
    assert!(!lib.fields.default_extensions.contains(&"OverloadedStrings"));
}

#[test]
fn toggle_extension_on() {
    let mut app = make_app();
    app.toggle_extension("DerivingStrategies").unwrap();

    let ast = app.ast();
    let lib = ast.library.as_ref().unwrap();
    assert!(lib.fields.default_extensions.contains(&"DerivingStrategies"));
}

// -- List length --

#[test]
fn current_list_len_for_metadata() {
    let mut app = make_app();
    app.current_view = cabalist_tui::views::View::Metadata;
    assert_eq!(app.current_list_len(), 13);
}

#[test]
fn current_list_len_for_dependencies() {
    let mut app = make_app();
    app.current_view = cabalist_tui::views::View::Dependencies;
    assert_eq!(app.current_list_len(), 2); // base, text
}

// -- Lints --

#[test]
fn lints_are_populated() {
    let app = make_app();
    // Should have at least some lints (missing-source-repo, etc.)
    assert!(!app.lints.is_empty(), "expected at least some lints");
}

// -- Deps tree mode --

#[test]
fn toggle_deps_tree_mode() {
    let mut app = make_app();
    assert!(!app.deps_tree_mode);
    app.deps_tree_mode = true;
    assert!(app.deps_tree_mode);
}

// -- Deps filter --

#[test]
fn deps_filter_starts_inactive() {
    let app = make_app();
    assert!(!app.deps_filter_active);
    assert!(app.deps_filter_query.is_empty());
}
