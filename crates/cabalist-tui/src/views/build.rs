//! Build output view — shows streaming build/test output.

use crate::app::App;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

/// Render the build output view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let status = if app.build_running {
        " Build Output (running...) "
    } else {
        " Build Output "
    };

    let block = Block::default()
        .title(status)
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_lines = inner.height as usize;

    let mut lines: Vec<Line<'_>> = Vec::new();

    if app.build_output.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No build output yet.",
            theme.muted_style(),
        )));
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "  Press [b] to build, [t] to test, [c] to clean.",
            theme.muted_style(),
        )));
    } else {
        // If the user has scrolled, use the scroll offset; otherwise
        // auto-scroll to the bottom (follow mode).
        let total = app.build_output.len();
        let auto_start = total.saturating_sub(max_lines);
        let start = if app.build_scroll > 0 && app.build_scroll < auto_start {
            app.build_scroll
        } else {
            auto_start
        };
        let end = (start + max_lines).min(total);
        for line in &app.build_output[start..end] {
            let style = if line.contains("error") || line.contains("Error") {
                theme.error_style()
            } else if line.contains("warning") || line.contains("Warning") {
                theme.warning_style()
            } else if line.contains("succeeded") || line.contains("Succeeded") {
                theme.success_style()
            } else {
                theme.normal()
            };
            lines.push(Line::from(Span::styled(format!("  {line}"), style)));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
