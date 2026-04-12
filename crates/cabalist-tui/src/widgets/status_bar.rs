//! Bottom status/keybinding bar.

use crate::app::App;
use crate::views::View;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Render the status bar at the bottom of the screen.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    // If there is a recent status message, show it.
    if let Some((ref msg, ref instant)) = app.status_message {
        if instant.elapsed().as_secs() < 3 {
            let line = Line::from(vec![
                Span::styled(" ", theme.header()),
                Span::styled(msg.clone(), theme.header()),
                Span::styled(
                    " ".repeat(area.width.saturating_sub(msg.len() as u16 + 1) as usize),
                    theme.header(),
                ),
            ]);
            frame.render_widget(Paragraph::new(line), area);
            return;
        }
    }

    let keybindings = match app.current_view {
        View::Dashboard => " [d]eps  [e]xt  [b]uild  [m]eta  [p]roject  [i]nit  [?]help  [q]uit",
        View::Dependencies => {
            " [a]dd  [r]emove  [v]iew  [/]search  [Tab]component  [Esc]back  [?]help"
        }
        View::Extensions => " [Space]toggle  [/]search  [i]nfo  [Tab]component  [Esc]back  [?]help",
        View::Build => " [b]uild  [t]est  [c]lean  [Esc]back  [?]help",
        View::Metadata => " [j/k]navigate  [Esc]back  [?]help",
        View::Help => " Press any key to close",
        View::Project => " [j/k]navigate  [Enter]edit  [Esc]back  [?]help",
        View::Init => " [Enter]next  [Esc]back  [Tab]cycle option  [Ctrl+C]quit",
    };

    let padding = area.width.saturating_sub(keybindings.len() as u16);
    let line = Line::from(vec![
        Span::styled(keybindings, theme.header()),
        Span::styled(" ".repeat(padding as usize), theme.header()),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}
