//! Top header bar showing app name, active view, and project info.

use crate::app::App;
use crate::views::View;
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

    let view_name = match app.current_view {
        View::Dashboard => "Dashboard",
        View::Dependencies => "Dependencies",
        View::Extensions => "Extensions",
        View::Build => "Build",
        View::Metadata => "Metadata",
        View::Project => "Project",
        View::Help => "Help",
        View::Init => "Init",
    };

    let left = format!(" cabalist v{}", env!("CARGO_PKG_VERSION"));
    let center = format!(" {view_name} ");
    let right = format!("{project_name} {project_version}{dirty_marker} ");

    let total_len = left.len() + center.len() + right.len();
    let padding_total = (area.width as usize).saturating_sub(total_len);
    let pad_left = padding_total / 2;
    let pad_right = padding_total - pad_left;

    let line = Line::from(vec![
        Span::styled(left, theme.header()),
        Span::styled(" ".repeat(pad_left), theme.header()),
        Span::styled(center, theme.accent()),
        Span::styled(" ".repeat(pad_right), theme.header()),
        Span::styled(right, theme.header()),
    ]);

    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}
