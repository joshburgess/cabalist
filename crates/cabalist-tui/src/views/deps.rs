//! Dependency manager view — shows build-depends for the selected component.

use crate::app::App;
use cabalist_parser::ast::{CabalFile, VersionRange};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the dependency manager view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let ast = app.ast();

    let components = collect_component_names(&ast);
    let selected_idx = app
        .selected_component
        .min(components.len().saturating_sub(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // component tabs
            Constraint::Min(4),    // dependency list
        ])
        .split(area);

    // -- Component tabs --
    render_component_tabs(frame, app, &components, selected_idx, chunks[0]);

    // -- Dependency list --
    let deps = deps_for_component(&ast, selected_idx);
    render_dep_list(frame, app, &deps, chunks[1]);
}

/// Information about a dependency to display.
struct DepDisplay {
    package: String,
    version_str: String,
    has_upper_bound: bool,
}

fn collect_component_names(ast: &CabalFile<'_>) -> Vec<String> {
    let mut names = Vec::new();
    if ast.library.is_some() {
        names.push("library".to_string());
    }
    for exe in &ast.executables {
        names.push(format!("exe:{}", exe.fields.name.unwrap_or("unnamed")));
    }
    for ts in &ast.test_suites {
        names.push(format!("test:{}", ts.fields.name.unwrap_or("unnamed")));
    }
    for bm in &ast.benchmarks {
        names.push(format!("bench:{}", bm.fields.name.unwrap_or("unnamed")));
    }
    names
}

fn deps_for_component(ast: &CabalFile<'_>, idx: usize) -> Vec<DepDisplay> {
    let mut component_idx = 0usize;

    if let Some(ref lib) = ast.library {
        if component_idx == idx {
            return to_dep_displays(&lib.fields.build_depends);
        }
        component_idx += 1;
    }
    for exe in &ast.executables {
        if component_idx == idx {
            return to_dep_displays(&exe.fields.build_depends);
        }
        component_idx += 1;
    }
    for ts in &ast.test_suites {
        if component_idx == idx {
            return to_dep_displays(&ts.fields.build_depends);
        }
        component_idx += 1;
    }
    for bm in &ast.benchmarks {
        if component_idx == idx {
            return to_dep_displays(&bm.fields.build_depends);
        }
        component_idx += 1;
    }

    Vec::new()
}

fn to_dep_displays(deps: &[cabalist_parser::ast::Dependency<'_>]) -> Vec<DepDisplay> {
    deps.iter()
        .map(|d| {
            let version_str = d
                .version_range
                .as_ref()
                .map(|vr| vr.to_string())
                .unwrap_or_else(|| "(any)".to_string());
            let has_upper_bound = d
                .version_range
                .as_ref()
                .map(version_range_has_upper)
                .unwrap_or(false);
            DepDisplay {
                package: d.package.to_string(),
                version_str,
                has_upper_bound,
            }
        })
        .collect()
}

fn version_range_has_upper(vr: &VersionRange) -> bool {
    match vr {
        VersionRange::Any => false,
        VersionRange::Lt(_) | VersionRange::Lte(_) | VersionRange::MajorBound(_) => true,
        VersionRange::Eq(_) => true,
        VersionRange::Gt(_) | VersionRange::Gte(_) => false,
        VersionRange::And(a, b) => version_range_has_upper(a) || version_range_has_upper(b),
        VersionRange::Or(a, b) => version_range_has_upper(a) && version_range_has_upper(b),
        VersionRange::NoVersion => true,
    }
}

fn render_component_tabs(
    frame: &mut Frame,
    app: &App,
    components: &[String],
    selected: usize,
    area: Rect,
) {
    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans: Vec<Span<'_>> = Vec::new();
    for (i, name) in components.iter().enumerate() {
        let style = if i == selected {
            theme.selected()
        } else {
            theme.muted_style()
        };
        spans.push(Span::styled(format!(" {name} "), style));
        if i + 1 < components.len() {
            spans.push(Span::styled(" | ", theme.muted_style()));
        }
    }

    if components.is_empty() {
        spans.push(Span::styled(" (no components) ", theme.muted_style()));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, inner);
}

fn render_dep_list(frame: &mut Frame, app: &App, deps: &[DepDisplay], area: Rect) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" Dependencies ")
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    for (i, dep) in deps.iter().enumerate() {
        let is_selected = i == app.selected_index;
        let base_style = if is_selected {
            theme.selected()
        } else {
            theme.normal()
        };

        let bound_indicator = if dep.has_upper_bound {
            Span::styled(" + ", theme.success_style())
        } else {
            Span::styled(" ! ", theme.warning_style())
        };

        let pvp_label = if dep.has_upper_bound {
            Span::styled("PVP ok", theme.success_style())
        } else {
            Span::styled("no upper bound", theme.warning_style())
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {:<24}", dep.package), base_style),
            Span::styled(format!("{:<20}", dep.version_str), base_style),
            bound_indicator,
            pvp_label,
        ]));
    }

    if deps.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no dependencies)",
            theme.muted_style(),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
