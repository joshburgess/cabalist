//! Top header bar showing app name and project info.

use crate::app::App;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Render the header bar.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let ast = app.ast();
    let project_name = ast.name.unwrap_or("(no name)");
    let project_version = ast
        .version
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_default();

    let dirty_marker = if app.dirty { " [modified]" } else { "" };

    let left = format!(" cabalist v{}", env!("CARGO_PKG_VERSION"));
    let right = format!("{project_name} {project_version}{dirty_marker} ");

    let padding = area
        .width
        .saturating_sub(left.len() as u16 + right.len() as u16);

    let line = Line::from(vec![
        Span::styled(left, theme.header()),
        Span::styled(" ".repeat(padding as usize), theme.header()),
        Span::styled(right, theme.header()),
    ]);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
