//! Dashboard (home screen) — project overview, components, and health summary.

use crate::app::App;
use cabalist_parser::diagnostic::Severity;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the dashboard view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let ast = app.ast();

    // Split into three vertical sections: metadata, components, health.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // metadata
            Constraint::Length(10), // components
            Constraint::Min(4),     // health / lints
        ])
        .split(area);

    // -- Metadata section --
    render_metadata(frame, app, &ast, chunks[0]);

    // -- Components section --
    render_components(frame, app, &ast, chunks[1]);

    // -- Health section --
    render_health(frame, app, chunks[2]);
}

fn render_metadata(
    frame: &mut Frame,
    app: &App,
    ast: &cabalist_parser::ast::CabalFile<'_>,
    area: Rect,
) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" Metadata ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    let name = ast.name.unwrap_or("(not set)");
    lines.push(metadata_line(theme, "name", name, true));

    let version_str = ast
        .version
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "(not set)".to_string());
    lines.push(metadata_line(
        theme,
        "version",
        &version_str,
        ast.version.is_some(),
    ));

    let license = ast.license.unwrap_or("(not set)");
    lines.push(metadata_line(
        theme,
        "license",
        license,
        ast.license.is_some(),
    ));

    let synopsis = ast.synopsis.unwrap_or("(missing)");
    lines.push(metadata_line(
        theme,
        "synopsis",
        synopsis,
        ast.synopsis.is_some(),
    ));

    let cabal_version = ast
        .cabal_version
        .as_ref()
        .map(|cv| cv.raw.to_string())
        .unwrap_or_else(|| "(not set)".to_string());
    lines.push(metadata_line(
        theme,
        "cabal-version",
        &cabal_version,
        ast.cabal_version.is_some(),
    ));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn metadata_line<'a>(
    theme: &crate::theme::Theme,
    label: &'a str,
    value: &'a str,
    ok: bool,
) -> Line<'a> {
    let indicator_style = if ok {
        theme.success_style()
    } else {
        theme.warning_style()
    };
    let indicator = if ok { " + " } else { " ! " };

    Line::from(vec![
        Span::styled(indicator, indicator_style),
        Span::styled(format!("{label}: "), theme.bold()),
        Span::styled(value, theme.normal()),
    ])
}

fn render_components(
    frame: &mut Frame,
    app: &App,
    ast: &cabalist_parser::ast::CabalFile<'_>,
    area: Rect,
) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" Components ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    if let Some(ref lib) = ast.library {
        let n_modules = lib.exposed_modules.len() + lib.fields.other_modules.len();
        let n_deps = lib.fields.build_depends.len();
        let name = lib.fields.name.unwrap_or("library");
        lines.push(component_line(theme, "lib", name, n_modules, n_deps));
    }

    for exe in &ast.executables {
        let name = exe.fields.name.unwrap_or("(unnamed)");
        let n_modules = exe.fields.other_modules.len();
        let n_deps = exe.fields.build_depends.len();
        lines.push(component_line(theme, "exe", name, n_modules, n_deps));
    }

    for ts in &ast.test_suites {
        let name = ts.fields.name.unwrap_or("(unnamed)");
        let n_modules = ts.fields.other_modules.len();
        let n_deps = ts.fields.build_depends.len();
        lines.push(component_line(theme, "test", name, n_modules, n_deps));
    }

    for bm in &ast.benchmarks {
        let name = bm.fields.name.unwrap_or("(unnamed)");
        let n_modules = bm.fields.other_modules.len();
        let n_deps = bm.fields.build_depends.len();
        lines.push(component_line(theme, "bench", name, n_modules, n_deps));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no components found)",
            theme.muted_style(),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn component_line<'a>(
    theme: &crate::theme::Theme,
    kind: &'a str,
    name: &'a str,
    modules: usize,
    deps: usize,
) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {kind:<6}"), theme.accent()),
        Span::styled(format!("{name:<24}"), theme.normal()),
        Span::styled(
            format!("{modules} modules, {deps} deps"),
            theme.muted_style(),
        ),
    ])
}

fn render_health(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" Health ")
        .borders(Borders::ALL)
        .border_style(theme.border_style());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let n_errors = app
        .lints
        .iter()
        .filter(|l| l.severity == Severity::Error)
        .count();
    let n_warnings = app
        .lints
        .iter()
        .filter(|l| l.severity == Severity::Warning)
        .count();
    let n_info = app
        .lints
        .iter()
        .filter(|l| l.severity == Severity::Info)
        .count();

    let mut lines: Vec<Line<'_>> = Vec::new();

    let summary = format!("  {n_errors} errors, {n_warnings} warnings, {n_info} suggestions");
    let summary_style = if n_errors > 0 {
        theme.error_style()
    } else if n_warnings > 0 {
        theme.warning_style()
    } else {
        theme.success_style()
    };
    lines.push(Line::from(Span::styled(summary, summary_style)));

    // Show up to 5 lint messages.
    for lint in app.lints.iter().take(5) {
        let sev_style = match lint.severity {
            Severity::Error => theme.error_style(),
            Severity::Warning => theme.warning_style(),
            Severity::Info => theme.muted_style(),
        };
        let prefix = match lint.severity {
            Severity::Error => "  E ",
            Severity::Warning => "  W ",
            Severity::Info => "  I ",
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, sev_style),
            Span::styled(lint.message.clone(), theme.normal()),
        ]));
    }

    if app.lints.len() > 5 {
        lines.push(Line::from(Span::styled(
            format!("  ... and {} more", app.lints.len() - 5),
            theme.muted_style(),
        )));
    }

    if app.lints.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No issues found!",
            theme.success_style(),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Extension trait to get a border style from the theme.
trait ThemeBorderExt {
    fn border_style(&self) -> ratatui::style::Style;
}

impl ThemeBorderExt for crate::theme::Theme {
    fn border_style(&self) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(self.border)
    }
}
