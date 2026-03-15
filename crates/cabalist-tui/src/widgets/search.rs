//! Inline search/filter popup overlay.

use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// Render the search popup as a centered overlay.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    // Center a box that is 60 columns wide and 5 rows tall.
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 5u16.min(area.height.saturating_sub(2));

    let popup_area = centered_rect(popup_width, popup_height, area);

    // Clear the area behind the popup.
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Search ")
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

    let help_line = Line::from(vec![Span::styled(
        " [Enter] confirm  [Esc] cancel",
        theme.muted_style(),
    )]);

    let text = vec![search_line, Line::default(), help_line];
    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

/// Return a centered `Rect` of the given size within `outer`.
fn centered_rect(width: u16, height: u16, outer: Rect) -> Rect {
    let x = outer.x + outer.width.saturating_sub(width) / 2;
    let y = outer.y + outer.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(outer.width), height.min(outer.height))
}
