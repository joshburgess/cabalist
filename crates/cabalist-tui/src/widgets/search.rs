//! Inline search/filter popup overlay.

use crate::app::App;
use crate::views::View;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Render the search popup as a centered overlay.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let is_deps_view = app.current_view == View::Dependencies;

    // Compute popup height based on whether we show results.
    let result_lines = if is_deps_view {
        // Reserve space for results: up to 10 results + 1 blank line + 1 help line.
        let count = app.search_results.len();
        if app.hackage_index.is_none() {
            // Show "(Hackage index not available)" line.
            1
        } else if app.search_query.len() < 2 {
            // Show "Type to search..." line.
            1
        } else if count > 0 {
            count
        } else {
            // Show "No results" line.
            1
        }
    } else {
        0
    };

    // search line + blank + result lines + blank + help line = result_lines + 4
    let popup_height = if is_deps_view {
        (result_lines as u16 + 5).min(area.height.saturating_sub(2))
    } else {
        5u16.min(area.height.saturating_sub(2))
    };
    let popup_width = 60u16.min(area.width.saturating_sub(4));

    let popup_area = centered_rect(popup_width, popup_height, area);

    // Clear the area behind the popup.
    frame.render_widget(Clear, popup_area);

    let title = if is_deps_view {
        " Add Dependency "
    } else {
        " Search "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.accent());

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let cursor_char = "_";
    let search_line = Line::from(vec![
        Span::styled(" > ", theme.accent()),
        Span::raw(&app.search_query),
        Span::styled(
            cursor_char,
            theme.accent().add_modifier(Modifier::SLOW_BLINK),
        ),
    ]);

    let mut text = vec![search_line, Line::default()];

    if is_deps_view {
        if app.hackage_index.is_none() {
            text.push(Line::from(Span::styled(
                "  Hackage index not available.",
                theme.warning_style(),
            )));
            text.push(Line::from(Span::styled(
                "  Press Ctrl+U to download it, or run: cabalist-cli update-index",
                theme.muted_style(),
            )));
        } else if app.search_query.len() < 2 {
            text.push(Line::from(Span::styled(
                "  Type to search...",
                theme.muted_style(),
            )));
        } else if app.search_results.is_empty() {
            text.push(Line::from(Span::styled(
                "  No results",
                theme.muted_style(),
            )));
        } else {
            for (i, result) in app.search_results.iter().enumerate() {
                let is_selected = i == app.search_selected;
                let style = if is_selected {
                    theme.selected()
                } else {
                    theme.normal()
                };

                // Format: "  package-name  version  synopsis"
                let version_str = result
                    .package
                    .latest_version()
                    .map(|v| v.to_string())
                    .unwrap_or_default();

                let synopsis = result.package.synopsis.as_str();
                // Truncate synopsis to fit within popup width.
                let name_len = result.package.name.len();
                let ver_len = version_str.len();
                // 2 leading spaces + name + 2 spaces + version + 2 spaces + synopsis
                let used = 2 + name_len + 2 + ver_len + 2;
                let max_synopsis = (popup_width as usize).saturating_sub(used + 2); // account for borders
                let truncated_synopsis = if synopsis.len() > max_synopsis {
                    &synopsis[..max_synopsis.min(synopsis.len())]
                } else {
                    synopsis
                };

                let line = Line::from(vec![
                    Span::styled("  ", style),
                    Span::styled(result.package.name.clone(), style.add_modifier(Modifier::BOLD)),
                    Span::styled("  ", style),
                    Span::styled(version_str, style),
                    Span::styled("  ", style),
                    Span::styled(truncated_synopsis.to_string(), if is_selected { style } else { theme.muted_style() }),
                ]);
                text.push(line);
            }
        }
        text.push(Line::default());
    }

    let help_line = Line::from(vec![Span::styled(
        " [Enter] confirm  [Esc] cancel",
        theme.muted_style(),
    )]);
    text.push(help_line);

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

/// Return a centered `Rect` of the given size within `outer`.
fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let x = outer.x + outer.width.saturating_sub(width) / 2;
    let y = outer.y + outer.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(outer.width), height.min(outer.height))
}
