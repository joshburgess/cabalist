//! Extension browser/toggler view.

use crate::app::App;
use cabalist_ghc::extensions::load_extensions;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the extensions view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let ast = app.ast();

    // Gather extensions currently enabled in the library (or first component).
    let enabled: Vec<&str> = if let Some(ref lib) = ast.library {
        lib.fields.default_extensions.clone()
    } else {
        Vec::new()
    };

    let all_extensions = load_extensions();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title / filter status
            Constraint::Min(4),    // extension list
        ])
        .split(area);

    // Title bar.
    let title_block = Block::default()
        .title(format!(
            " GHC Extensions ({} available) ",
            all_extensions.len()
        ))
        .borders(Borders::BOTTOM)
        .border_style(ratatui::style::Style::default().fg(theme.border));
    frame.render_widget(title_block, chunks[0]);

    // Extension list.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);

    let filter = if app.search_active || !app.search_query.is_empty() {
        Some(app.search_query.to_lowercase())
    } else {
        None
    };

    let mut lines: Vec<Line<'_>> = Vec::new();

    // First show enabled extensions.
    let mut shown_enabled = false;
    for ext_name in &enabled {
        if let Some(ref f) = filter {
            if !ext_name.to_lowercase().contains(f.as_str()) {
                continue;
            }
        }
        if !shown_enabled {
            lines.push(Line::from(Span::styled(
                "  Enabled (project-wide):",
                theme.bold(),
            )));
            shown_enabled = true;
        }
        let desc = all_extensions
            .iter()
            .find(|e| e.name.as_str() == *ext_name)
            .map(|e| e.description.as_str())
            .unwrap_or("");

        lines.push(extension_line(theme, ext_name, desc, true, false));
    }

    if shown_enabled {
        lines.push(Line::default());
    }

    // Then show available (not enabled) extensions.
    lines.push(Line::from(Span::styled("  Available:", theme.bold())));

    let enabled_set: std::collections::HashSet<&str> = enabled.iter().copied().collect();

    let mut count = 0usize;
    let max_display = inner.height.saturating_sub(lines.len() as u16 + 1) as usize;

    for ext in all_extensions.iter() {
        if enabled_set.contains(ext.name.as_str()) {
            continue;
        }
        if let Some(ref f) = filter {
            if !ext.name.to_lowercase().contains(f.as_str())
                && !ext.description.to_lowercase().contains(f.as_str())
            {
                continue;
            }
        }
        if count >= max_display {
            lines.push(Line::from(Span::styled(
                "  ... (use / to filter)",
                theme.muted_style(),
            )));
            break;
        }
        lines.push(extension_line(
            theme,
            &ext.name,
            &ext.description,
            false,
            ext.recommended,
        ));
        count += 1;
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn extension_line<'a>(
    theme: &crate::theme::Theme,
    name: &'a str,
    description: &'a str,
    enabled: bool,
    recommended: bool,
) -> Line<'a> {
    let checkbox = if enabled { "[x]" } else { "[ ]" };
    let cb_style = if enabled {
        theme.success_style()
    } else {
        theme.muted_style()
    };

    let rec_badge = if recommended && !enabled {
        Span::styled(" (rec) ", theme.accent())
    } else {
        Span::raw("")
    };

    // Truncate description to keep lines reasonable.
    let desc_truncated: String = if description.len() > 40 {
        format!("{}...", &description[..37])
    } else {
        description.to_string()
    };

    Line::from(vec![
        Span::styled(format!("    {checkbox} "), cb_style),
        Span::styled(format!("{name:<28}"), theme.normal()),
        rec_badge,
        Span::styled(desc_truncated, theme.muted_style()),
    ])
}
