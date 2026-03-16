//! Metadata editor view — top-level package metadata fields.

use crate::app::App;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Metadata fields displayed in this view (label, extractor).
pub static METADATA_FIELDS: &[&str] = &[
    "name",
    "version",
    "cabal-version",
    "license",
    "author",
    "maintainer",
    "homepage",
    "bug-reports",
    "synopsis",
    "description",
    "category",
    "build-type",
    "tested-with",
];

/// Render the metadata editor view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let ast = app.ast();

    let block = Block::default()
        .title(" Package Metadata ")
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area into the field list and a hint bar at the bottom.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let list_area = chunks[0];
    let hint_area = chunks[1];

    let mut lines: Vec<Line<'_>> = Vec::new();

    for (i, &field_name) in METADATA_FIELDS.iter().enumerate() {
        let is_selected = i == app.selected_index;

        let value = match field_name {
            "name" => ast.name.map(|s| s.to_string()),
            "version" => ast.version.as_ref().map(|v| v.to_string()),
            "cabal-version" => ast.cabal_version.as_ref().map(|cv| cv.raw.to_string()),
            "license" => ast.license.map(|s| s.to_string()),
            "author" => ast.author.map(|s| s.to_string()),
            "maintainer" => ast.maintainer.map(|s| s.to_string()),
            "homepage" => ast.homepage.map(|s| s.to_string()),
            "bug-reports" => ast.bug_reports.map(|s| s.to_string()),
            "synopsis" => ast.synopsis.map(|s| s.to_string()),
            "description" => ast.description.map(|s| s.to_string()),
            "category" => ast.category.map(|s| s.to_string()),
            "build-type" => ast.build_type.map(|s| s.to_string()),
            "tested-with" => ast.tested_with.map(|s| s.to_string()),
            _ => None,
        };

        let display_value = value.unwrap_or_else(|| "(not set)".to_string());
        let has_value = display_value != "(not set)";

        // When editing the selected field, show the edit buffer instead.
        let is_editing = app.editing_metadata && is_selected;

        let base_style = if is_selected {
            theme.selected()
        } else {
            theme.normal()
        };

        let indicator = if is_editing {
            " > "
        } else if has_value {
            " + "
        } else {
            " - "
        };
        let ind_style = if is_selected {
            theme.selected()
        } else if has_value {
            theme.success_style()
        } else {
            theme.muted_style()
        };

        let value_text: String = if is_editing {
            // Show the edit buffer with a block cursor.
            format!("{}_", app.metadata_edit_buffer)
        } else {
            // Truncate long values.
            if display_value.len() > 50 {
                format!("{}...", &display_value[..47])
            } else {
                display_value
            }
        };

        lines.push(Line::from(vec![
            Span::styled(indicator, ind_style),
            Span::styled(format!("{field_name:<16}"), base_style),
            Span::styled(value_text, base_style),
        ]));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, list_area);

    // Render hint bar.
    let hint_text = if app.editing_metadata {
        "[Enter] save  [Esc] cancel"
    } else {
        "[Enter] edit  [Esc] back  [j/k] navigate"
    };
    let hint = Paragraph::new(Line::from(Span::styled(hint_text, theme.muted_style())));
    frame.render_widget(hint, hint_area);
}
