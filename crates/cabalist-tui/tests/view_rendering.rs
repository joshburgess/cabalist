//! Headless rendering tests for the TUI views.
//!
//! Uses ratatui's TestBackend to render each view and verify the output
//! contains expected content, catching rendering regressions.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

/// Create a test app with a sample .cabal file.
fn make_test_app() -> cabalist_tui::app::App {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("cabalist-tui-test-render-{id}"));
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

common warnings
  ghc-options: -Wall

library
  import: warnings
  exposed-modules: Lib
  build-depends:
    base ^>=4.17,
    text ^>=2.0,
    aeson ^>=2.2
  default-language: GHC2021

executable test-exe
  import: warnings
  main-is: Main.hs
  hs-source-dirs: app
  build-depends:
    base ^>=4.17,
    test-pkg
  default-language: GHC2021

test-suite test-tests
  import: warnings
  type: exitcode-stdio-1.0
  main-is: Main.hs
  hs-source-dirs: test
  build-depends:
    base ^>=4.17,
    test-pkg
  default-language: GHC2021
";
    std::fs::write(&cabal_path, source).unwrap();

    let theme = cabalist_tui::theme::Theme::dark();
    cabalist_tui::app::App::new(cabal_path, theme).unwrap()
}

/// Render the app to a buffer and return the text content.
fn render_to_string(app: &cabalist_tui::app::App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let area = frame.area();
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                    ratatui::layout::Constraint::Length(1),
                ])
                .split(area);

            cabalist_tui::widgets::header::render(frame, app, chunks[0]);

            match app.current_view {
                cabalist_tui::views::View::Dashboard => {
                    cabalist_tui::views::dashboard::render(frame, app, chunks[1]);
                }
                cabalist_tui::views::View::Dependencies => {
                    cabalist_tui::views::deps::render(frame, app, chunks[1]);
                }
                cabalist_tui::views::View::Extensions => {
                    cabalist_tui::views::extensions::render(frame, app, chunks[1]);
                }
                cabalist_tui::views::View::Metadata => {
                    cabalist_tui::views::metadata::render(frame, app, chunks[1]);
                }
                cabalist_tui::views::View::Build => {
                    cabalist_tui::views::build::render(frame, app, chunks[1]);
                }
                cabalist_tui::views::View::Project => {
                    cabalist_tui::views::project::render(frame, app, chunks[1]);
                }
                _ => {}
            }

            cabalist_tui::widgets::status_bar::render(frame, app, chunks[2]);
        })
        .unwrap();

    let backend = terminal.backend();
    let buf = backend.buffer();
    let mut output = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            output.push_str(cell.symbol());
        }
        output.push('\n');
    }
    output
}

#[test]
fn dashboard_renders_package_name() {
    let app = make_test_app();
    let output = render_to_string(&app, 80, 24);
    assert!(
        output.contains("test-pkg"),
        "Dashboard should show the package name.\nOutput:\n{output}"
    );
}

#[test]
fn dashboard_renders_version() {
    let app = make_test_app();
    let output = render_to_string(&app, 80, 24);
    assert!(
        output.contains("0.1.0.0"),
        "Dashboard should show the version.\nOutput:\n{output}"
    );
}

#[test]
fn deps_view_shows_all_dependencies() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Dependencies;
    let output = render_to_string(&app, 80, 24);
    // Verify all 3 dependencies are shown.
    assert!(
        output.contains("base"),
        "Should show 'base'.\nOutput:\n{output}"
    );
    assert!(
        output.contains("text"),
        "Should show 'text'.\nOutput:\n{output}"
    );
    assert!(
        output.contains("aeson"),
        "Should show 'aeson'.\nOutput:\n{output}"
    );
    // Verify version constraints are displayed.
    assert!(
        output.contains("^>=4.17"),
        "Should show base constraint.\nOutput:\n{output}"
    );
    assert!(
        output.contains("^>=2.0"),
        "Should show text constraint.\nOutput:\n{output}"
    );
    assert!(
        output.contains("^>=2.2"),
        "Should show aeson constraint.\nOutput:\n{output}"
    );
}

#[test]
fn deps_view_shows_pvp_status() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Dependencies;
    let output = render_to_string(&app, 80, 24);
    assert!(
        output.contains("PVP ok"),
        "Deps view should show PVP status.\nOutput:\n{output}"
    );
}

#[test]
fn deps_tree_view_renders() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Dependencies;
    app.deps_tree_mode = true;
    let output = render_to_string(&app, 80, 24);
    assert!(
        output.contains("Dependency Tree"),
        "Tree view should show its title.\nOutput:\n{output}"
    );
}

#[test]
fn metadata_view_shows_all_fields() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Metadata;
    let output = render_to_string(&app, 80, 24);
    // Verify key field labels are present.
    assert!(
        output.contains("name"),
        "Should show 'name' label.\nOutput:\n{output}"
    );
    assert!(
        output.contains("version"),
        "Should show 'version' label.\nOutput:\n{output}"
    );
    assert!(
        output.contains("license"),
        "Should show 'license' label.\nOutput:\n{output}"
    );
    // Verify values are displayed.
    assert!(
        output.contains("test-pkg"),
        "Should show package name value.\nOutput:\n{output}"
    );
    assert!(
        output.contains("MIT"),
        "Should show license value.\nOutput:\n{output}"
    );
    assert!(
        output.contains("0.1.0.0"),
        "Should show version value.\nOutput:\n{output}"
    );
    assert!(
        output.contains("A test package"),
        "Should show synopsis.\nOutput:\n{output}"
    );
}

#[test]
fn extensions_view_renders() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Extensions;
    let output = render_to_string(&app, 80, 24);
    // Should show either enabled extensions or available ones.
    assert!(
        output.contains("Extensions") || output.contains("extension"),
        "Extensions view should have a title.\nOutput:\n{output}"
    );
}

#[test]
fn build_view_renders_empty() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Build;
    let output = render_to_string(&app, 80, 24);
    assert!(
        output.contains("Build") || output.contains("build"),
        "Build view should have a title.\nOutput:\n{output}"
    );
}

#[test]
fn project_view_renders_no_project() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Project;
    let output = render_to_string(&app, 80, 24);
    assert!(
        output.contains("cabal.project") || output.contains("No cabal.project"),
        "Project view should mention cabal.project.\nOutput:\n{output}"
    );
}

#[test]
fn status_bar_shows_keybindings() {
    let app = make_test_app();
    let output = render_to_string(&app, 100, 24);
    assert!(
        output.contains("[d]eps") || output.contains("deps"),
        "Status bar should show view keybindings.\nOutput:\n{output}"
    );
}

#[test]
fn header_shows_package_in_header() {
    let app = make_test_app();
    let output = render_to_string(&app, 80, 24);
    assert!(
        output.contains("test-pkg"),
        "Header should show the package name.\nOutput:\n{output}"
    );
}

#[test]
fn dashboard_shows_component_counts() {
    let app = make_test_app();
    let output = render_to_string(&app, 80, 24);
    // Should show library with 3 deps and exe/test with 2 deps each.
    assert!(
        output.contains("3 deps"),
        "Should show library dep count.\nOutput:\n{output}"
    );
    assert!(
        output.contains("test-exe"),
        "Should show executable name.\nOutput:\n{output}"
    );
    assert!(
        output.contains("test-tests"),
        "Should show test-suite name.\nOutput:\n{output}"
    );
}

#[test]
fn dashboard_shows_health_summary() {
    let app = make_test_app();
    let output = render_to_string(&app, 80, 24);
    // Should show some lint summary (errors, warnings, suggestions).
    assert!(
        output.contains("errors") || output.contains("warnings") || output.contains("suggestions"),
        "Should show health summary.\nOutput:\n{output}"
    );
}

#[test]
fn deps_view_shows_correct_dep_count() {
    let mut app = make_test_app();
    app.current_view = cabalist_tui::views::View::Dependencies;
    // Library has 3 deps (base, text, aeson).
    assert_eq!(app.current_list_len(), 3);
}

#[test]
fn small_terminal_does_not_crash() {
    let app = make_test_app();
    // Render at very small size — should not panic.
    let output = render_to_string(&app, 20, 5);
    assert!(!output.is_empty());
}
