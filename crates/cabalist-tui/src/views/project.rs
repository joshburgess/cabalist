//! cabal.project file viewer — shows parsed project configuration.

use crate::app::App;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the cabal.project view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let project = match &app.cabal_project {
        Some(proj) => proj,
        None => {
            let block = Block::default()
                .title(" cabal.project ")
                .borders(Borders::ALL)
                .border_style(ratatui::style::Style::default().fg(theme.border));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let msg = Paragraph::new(Line::from(Span::styled(
                "  No cabal.project file found in this project.",
                theme.muted_style(),
            )));
            frame.render_widget(msg, inner);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // project overview
            Constraint::Min(4),   // details
        ])
        .split(area);

    // -- Overview section --
    render_overview(frame, app, project, chunks[0]);

    // -- Detail sections --
    render_details(frame, app, project, chunks[1]);
}

fn render_overview(
    frame: &mut Frame,
    app: &App,
    project: &cabalist_project::CabalProject,
    area: Rect,
) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" cabal.project — Overview ")
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Packages (informational, not editable).
    let pkg_count = project.packages.len();
    let pkg_label = if pkg_count == 1 { "package" } else { "packages" };
    lines.push(Line::from(vec![
        Span::styled("  Packages:     ", theme.muted_style()),
        Span::styled(
            format!("{pkg_count} {pkg_label}"),
            theme.normal(),
        ),
        Span::styled(
            format!("  ({})", project.packages.join(", ")),
            theme.muted_style(),
        ),
    ]));

    // Editable fields: with-compiler, index-state.
    let field_values: [(&str, String); 2] = [
        (
            "Compiler:   ",
            project
                .with_compiler
                .clone()
                .unwrap_or_else(|| "(default)".to_string()),
        ),
        (
            "Index state:",
            project
                .index_state
                .clone()
                .unwrap_or_else(|| "(not pinned)".to_string()),
        ),
    ];

    for (fi, (label, value)) in field_values.iter().enumerate() {
        let is_selected = fi == app.selected_index;

        if is_selected && app.editing_project_field {
            // Editing mode.
            let indicator = Span::styled("  > ", theme.accent());
            let label_span = Span::styled(format!("{label} "), theme.label_style());
            let value_span = Span::styled(
                format!("{}\u{2588}", app.project_edit_buffer),
                theme.accent(),
            );
            lines.push(Line::from(vec![indicator, label_span, value_span]));
        } else {
            let indicator = if is_selected {
                Span::styled("  > ", theme.accent())
            } else {
                Span::styled("    ", theme.normal())
            };
            let style = if is_selected {
                theme.selected()
            } else {
                theme.normal()
            };
            lines.push(Line::from(vec![
                indicator,
                Span::styled(format!("{label} "), theme.label_style()),
                Span::styled(value.clone(), style),
            ]));
        }
    }

    // Source repo packages count.
    if !project.source_repo_packages.is_empty() {
        let count = project.source_repo_packages.len();
        let label = if count == 1 {
            "source-repository-package"
        } else {
            "source-repository-packages"
        };
        lines.push(Line::from(vec![
            Span::styled("    Repos:      ", theme.muted_style()),
            Span::styled(format!("{count} {label}"), theme.normal()),
        ]));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn render_details(
    frame: &mut Frame,
    app: &App,
    project: &cabalist_project::CabalProject,
    area: Rect,
) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" Details ")
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut item_idx = OVERVIEW_FIELDS.len(); // offset past overview editable fields

    // Constraints.
    if !project.constraints.is_empty() {
        let is_selected = item_idx == app.selected_index;
        let style = if is_selected { theme.selected() } else { theme.label_style() };
        lines.push(Line::from(Span::styled("  Constraints:", style)));
        item_idx += 1;
        for c in &project.constraints {
            let is_selected = item_idx == app.selected_index;
            let style = if is_selected { theme.selected() } else { theme.normal() };
            lines.push(Line::from(Span::styled(format!("    {c}"), style)));
            item_idx += 1;
        }
        lines.push(Line::default());
    }

    // Allow-newer.
    if !project.allow_newer.is_empty() {
        let is_selected = item_idx == app.selected_index;
        let style = if is_selected { theme.selected() } else { theme.label_style() };
        lines.push(Line::from(Span::styled("  Allow-newer:", style)));
        item_idx += 1;
        for a in &project.allow_newer {
            let is_selected = item_idx == app.selected_index;
            let style = if is_selected { theme.selected() } else { theme.warning_style() };
            lines.push(Line::from(Span::styled(format!("    {a}"), style)));
            item_idx += 1;
        }
        lines.push(Line::default());
    }

    // Allow-older.
    if !project.allow_older.is_empty() {
        let is_selected = item_idx == app.selected_index;
        let style = if is_selected { theme.selected() } else { theme.label_style() };
        lines.push(Line::from(Span::styled("  Allow-older:", style)));
        item_idx += 1;
        for a in &project.allow_older {
            let is_selected = item_idx == app.selected_index;
            let style = if is_selected { theme.selected() } else { theme.warning_style() };
            lines.push(Line::from(Span::styled(format!("    {a}"), style)));
            item_idx += 1;
        }
        lines.push(Line::default());
    }

    // Source repository packages.
    if !project.source_repo_packages.is_empty() {
        let is_selected = item_idx == app.selected_index;
        let style = if is_selected { theme.selected() } else { theme.label_style() };
        lines.push(Line::from(Span::styled("  Source Repository Packages:", style)));
        item_idx += 1;
        for repo in &project.source_repo_packages {
            let is_selected = item_idx == app.selected_index;
            let style = if is_selected { theme.selected() } else { theme.normal() };
            let loc = repo.location.as_deref().unwrap_or("(no location)");
            let tag = repo.tag.as_deref().map(|t| format!(" @ {t}")).unwrap_or_default();
            lines.push(Line::from(Span::styled(
                format!("    {loc}{tag}"),
                style,
            )));
            item_idx += 1;
        }
        lines.push(Line::default());
    }

    // Package stanzas.
    if !project.package_stanzas.is_empty() {
        let is_selected = item_idx == app.selected_index;
        let style = if is_selected { theme.selected() } else { theme.label_style() };
        lines.push(Line::from(Span::styled("  Package Stanzas:", style)));
        item_idx += 1;
        for stanza in &project.package_stanzas {
            let is_selected = item_idx == app.selected_index;
            let style = if is_selected { theme.selected() } else { theme.normal() };
            let field_count = stanza.fields.len();
            let label = if field_count == 1 { "field" } else { "fields" };
            lines.push(Line::from(Span::styled(
                format!("    package {} ({field_count} {label})", stanza.name),
                style,
            )));
            item_idx += 1;
        }
        lines.push(Line::default());
    }

    // Other fields.
    if !project.other_fields.is_empty() {
        let is_selected = item_idx == app.selected_index;
        let style = if is_selected { theme.selected() } else { theme.label_style() };
        lines.push(Line::from(Span::styled("  Other Fields:", style)));
        item_idx += 1;
        for (key, value) in &project.other_fields {
            let is_selected = item_idx == app.selected_index;
            let style = if is_selected { theme.selected() } else { theme.normal() };
            let truncated = if value.len() > 50 {
                format!("{}...", &value[..47])
            } else {
                value.clone()
            };
            lines.push(Line::from(Span::styled(
                format!("    {key}: {truncated}"),
                style,
            )));
            item_idx += 1;
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no additional configuration)",
            theme.muted_style(),
        )));
    }

    let _ = item_idx; // suppress unused warning

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// The editable overview fields and their indices.
/// These are the fields shown in the overview section and editable with Enter.
const OVERVIEW_FIELDS: &[(&str, &str)] = &[
    ("with-compiler", "Compiler"),
    ("index-state", "Index state"),
];

/// Count the total number of navigable items in the project view.
/// Includes overview editable fields + detail items.
pub fn item_count(project: &cabalist_project::CabalProject) -> usize {
    let mut count = OVERVIEW_FIELDS.len(); // editable overview fields

    if !project.constraints.is_empty() {
        count += 1 + project.constraints.len();
    }
    if !project.allow_newer.is_empty() {
        count += 1 + project.allow_newer.len();
    }
    if !project.allow_older.is_empty() {
        count += 1 + project.allow_older.len();
    }
    if !project.source_repo_packages.is_empty() {
        count += 1 + project.source_repo_packages.len();
    }
    if !project.package_stanzas.is_empty() {
        count += 1 + project.package_stanzas.len();
    }
    if !project.other_fields.is_empty() {
        count += 1 + project.other_fields.len();
    }

    count
}

/// Return the editable field name and current value at the given selection index,
/// or None if the item is not editable (e.g. a section header).
pub fn editable_field_at(
    project: &cabalist_project::CabalProject,
    index: usize,
) -> Option<(String, String)> {
    // First OVERVIEW_FIELDS.len() items are the editable overview fields.
    if index < OVERVIEW_FIELDS.len() {
        let (field_name, _label) = OVERVIEW_FIELDS[index];
        let value = match field_name {
            "with-compiler" => project.with_compiler.clone().unwrap_or_default(),
            "index-state" => project.index_state.clone().unwrap_or_default(),
            _ => return None,
        };
        return Some((field_name.to_string(), value));
    }
    None
}
