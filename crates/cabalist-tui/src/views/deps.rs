//! Dependency manager view — shows build-depends for the selected component.
//!
//! Supports two display modes toggled with 'v':
//! - **List mode** (default): flat list with PVP status and outdated indicators.
//! - **Tree mode**: ASCII tree showing components and their direct dependencies.

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

    if app.deps_tree_mode {
        render_tree_mode(frame, app, &ast, &components, area);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // component tabs
                Constraint::Min(4),   // dependency list
            ])
            .split(area);

        render_component_tabs(frame, app, &components, selected_idx, chunks[0]);

        let mut deps = deps_for_component(&ast, selected_idx, app);

        // Apply inline filter.
        if app.deps_filter_active && !app.deps_filter_query.is_empty() {
            let query = app.deps_filter_query.to_ascii_lowercase();
            deps.retain(|d| d.package.to_ascii_lowercase().contains(&query));
        }

        render_dep_list(frame, app, &deps, chunks[1]);
    }
}

/// Information about a dependency to display.
struct DepDisplay {
    package: String,
    version_str: String,
    has_upper_bound: bool,
    /// Latest version on Hackage, if known and newer than the constraint allows.
    outdated_latest: Option<String>,
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

fn deps_for_component(ast: &CabalFile<'_>, idx: usize, app: &App) -> Vec<DepDisplay> {
    let mut component_idx = 0usize;

    if let Some(ref lib) = ast.library {
        if component_idx == idx {
            return to_dep_displays(&lib.fields.build_depends, app);
        }
        component_idx += 1;
    }
    for exe in &ast.executables {
        if component_idx == idx {
            return to_dep_displays(&exe.fields.build_depends, app);
        }
        component_idx += 1;
    }
    for ts in &ast.test_suites {
        if component_idx == idx {
            return to_dep_displays(&ts.fields.build_depends, app);
        }
        component_idx += 1;
    }
    for bm in &ast.benchmarks {
        if component_idx == idx {
            return to_dep_displays(&bm.fields.build_depends, app);
        }
        component_idx += 1;
    }

    Vec::new()
}

fn to_dep_displays(deps: &[cabalist_parser::ast::Dependency<'_>], app: &App) -> Vec<DepDisplay> {
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

            // Check if outdated against Hackage.
            let outdated_latest = check_outdated(d.package, &d.version_range, app);

            DepDisplay {
                package: d.package.to_string(),
                version_str,
                has_upper_bound,
                outdated_latest,
            }
        })
        .collect()
}

/// Check if a dependency's constraint excludes the latest Hackage version.
fn check_outdated(
    package: &str,
    version_range: &Option<VersionRange>,
    app: &App,
) -> Option<String> {
    let index = app.hackage_index.as_ref()?;
    let latest = index.latest_version(package)?;

    // Convert hackage Version to parser Version for comparison.
    let parser_version = cabalist_parser::ast::Version {
        components: latest.components.clone(),
    };

    let is_constrained = match version_range {
        Some(vr) => !version_satisfies_range(&parser_version, vr),
        None => false, // "any" accepts everything
    };

    if is_constrained {
        Some(latest.to_string())
    } else {
        None
    }
}

/// Check if a version satisfies a version range (best-effort).
fn version_satisfies_range(
    version: &cabalist_parser::ast::Version,
    vr: &VersionRange,
) -> bool {
    use std::cmp::Ordering;

    let cmp_versions =
        |a: &cabalist_parser::ast::Version, b: &cabalist_parser::ast::Version| -> Ordering {
            let max_len = a.components.len().max(b.components.len());
            for i in 0..max_len {
                let ac = a.components.get(i).copied().unwrap_or(0);
                let bc = b.components.get(i).copied().unwrap_or(0);
                match ac.cmp(&bc) {
                    Ordering::Equal => continue,
                    other => return other,
                }
            }
            Ordering::Equal
        };

    match vr {
        VersionRange::Any => true,
        VersionRange::NoVersion => false,
        VersionRange::Eq(v) => cmp_versions(version, v) == Ordering::Equal,
        VersionRange::Gt(v) => cmp_versions(version, v) == Ordering::Greater,
        VersionRange::Gte(v) => cmp_versions(version, v) != Ordering::Less,
        VersionRange::Lt(v) => cmp_versions(version, v) == Ordering::Less,
        VersionRange::Lte(v) => cmp_versions(version, v) != Ordering::Greater,
        VersionRange::MajorBound(v) => {
            if cmp_versions(version, v) == Ordering::Less {
                return false;
            }
            let mut upper = v.clone();
            if upper.components.len() >= 2 {
                upper.components[1] += 1;
                upper.components.truncate(2);
            } else if upper.components.len() == 1 {
                upper.components[0] += 1;
            }
            cmp_versions(version, &upper) == Ordering::Less
        }
        VersionRange::And(a, b) => {
            version_satisfies_range(version, a) && version_satisfies_range(version, b)
        }
        VersionRange::Or(a, b) => {
            version_satisfies_range(version, a) || version_satisfies_range(version, b)
        }
    }
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
    let title = if app.deps_filter_active {
        format!(" Dependencies — filter: {}█ ", app.deps_filter_query)
    } else {
        " Dependencies ".to_string()
    };
    let block = Block::default()
        .title(title)
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

        let mut spans = vec![
            Span::styled(format!("  {:<24}", dep.package), base_style),
            Span::styled(format!("{:<20}", dep.version_str), base_style),
            bound_indicator,
            pvp_label,
        ];

        // Show outdated indicator if we have Hackage data.
        if let Some(ref latest) = dep.outdated_latest {
            spans.push(Span::styled(
                format!("  -> {latest}"),
                theme.error_style(),
            ));
        }

        lines.push(Line::from(spans));
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

/// Render the tree mode — shows all components and their dependencies as an ASCII tree.
fn render_tree_mode(
    frame: &mut Frame,
    app: &App,
    ast: &CabalFile<'_>,
    components: &[String],
    area: Rect,
) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" Dependency Tree ")
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut flat_idx = 0usize;
    let all_components = ast.all_components();

    for (ci, comp) in all_components.iter().enumerate() {
        let is_last_comp = ci == all_components.len() - 1;
        let comp_prefix = if is_last_comp { "  └── " } else { "  ├── " };
        let comp_name = &components[ci.min(components.len().saturating_sub(1))];

        let is_selected = flat_idx == app.selected_index;
        let style = if is_selected {
            theme.selected()
        } else {
            theme.label_style()
        };
        lines.push(Line::from(Span::styled(
            format!("{comp_prefix}{comp_name}"),
            style,
        )));
        flat_idx += 1;

        let deps = &comp.fields().build_depends;
        let child_prefix = if is_last_comp { "      " } else { "  │   " };

        for (di, dep) in deps.iter().enumerate() {
            let is_last_dep = di == deps.len() - 1;
            let dep_branch = if is_last_dep { "└── " } else { "├── " };

            let version_str = dep
                .version_range
                .as_ref()
                .map(|vr| format!(" {vr}"))
                .unwrap_or_default();

            let is_selected = flat_idx == app.selected_index;
            let style = if is_selected {
                theme.selected()
            } else {
                theme.normal()
            };

            let outdated = check_outdated(dep.package, &dep.version_range, app);
            let mut spans = vec![Span::styled(
                format!("{child_prefix}{dep_branch}{}{version_str}", dep.package),
                style,
            )];

            if let Some(latest) = outdated {
                spans.push(Span::styled(
                    format!("  -> {latest}"),
                    theme.error_style(),
                ));
            }

            lines.push(Line::from(spans));
            flat_idx += 1;
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no components)",
            theme.muted_style(),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Public version of `version_satisfies_range` for use by other views.
pub fn version_satisfies_range_pub(
    version: &cabalist_parser::ast::Version,
    vr: &VersionRange,
) -> bool {
    version_satisfies_range(version, vr)
}

/// Count the total items shown in tree mode for navigation bounds.
pub fn tree_mode_item_count(ast: &CabalFile<'_>) -> usize {
    let mut count = 0;
    let components = ast.all_components();
    for comp in &components {
        count += 1; // component header
        count += comp.fields().build_depends.len();
    }
    count
}
