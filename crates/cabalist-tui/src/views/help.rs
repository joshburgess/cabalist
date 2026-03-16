//! Help overlay — shows context-sensitive keybinding reference.

use crate::app::App;
use crate::views::View;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

/// Render the help overlay on top of the current view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    // Size the popup to ~60x20 or smaller.
    let width = 62u16.min(area.width.saturating_sub(4));
    let height = 22u16.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(theme.accent());

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(" Global Keybindings", theme.bold())),
        help_line(theme, "q / Ctrl+C", "Quit"),
        help_line(theme, "Ctrl+S", "Save .cabal file"),
        help_line(theme, "Ctrl+R", "Reload .cabal file"),
        help_line(theme, "?", "Toggle this help"),
        help_line(theme, "Esc", "Go back / close popup"),
        help_line(theme, "j/k or Up/Down", "Navigate lists"),
        help_line(theme, "g/G", "Jump to top/bottom"),
        help_line(theme, "Tab/Shift+Tab", "Switch component"),
        help_line(theme, "/", "Search/filter"),
        Line::default(),
        // View-specific help.
        Line::from(Span::styled(" View-specific", theme.bold())),
    ];

    match app.current_view {
        View::Dashboard | View::Help => {
            lines.push(help_line(theme, "d", "Go to Dependencies"));
            lines.push(help_line(theme, "e", "Go to Extensions"));
            lines.push(help_line(theme, "b", "Go to Build"));
            lines.push(help_line(theme, "m", "Go to Metadata"));
        }
        View::Dependencies => {
            lines.push(help_line(theme, "a", "Add dependency"));
            lines.push(help_line(theme, "r", "Remove dependency"));
        }
        View::Extensions => {
            lines.push(help_line(theme, "Space", "Toggle extension"));
            lines.push(help_line(theme, "i", "Show extension info"));
        }
        View::Build => {
            lines.push(help_line(theme, "b", "Run cabal build"));
            lines.push(help_line(theme, "t", "Run cabal test"));
            lines.push(help_line(theme, "c", "Run cabal clean"));
        }
        View::Metadata => {
            lines.push(help_line(theme, "Enter", "Edit selected field"));
        }
        View::Init => {
            lines.push(help_line(theme, "Enter", "Confirm / next step"));
            lines.push(help_line(theme, "Esc", "Go back / cancel"));
            lines.push(help_line(theme, "Tab", "Cycle option"));
        }
    }

    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        " Press any key to close this help.",
        theme.muted_style(),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn help_line<'a>(theme: &crate::theme::Theme, key: &'a str, description: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("   {key:<20}"), theme.accent()),
        Span::styled(description, theme.normal()),
    ])
}
