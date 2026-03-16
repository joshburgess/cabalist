//! Build output view — shows streaming build/test output.

use crate::app::App;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

/// Render the build output view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let has_diagnostics = !app.build_diagnostics.is_empty() && !app.build_running;

    // Split the area: main output on top, diagnostic summary at bottom (if any).
    let chunks = if has_diagnostics {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(5)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(0)])
            .split(area)
    };

    // --- Build output section ---
    let status = if app.build_running {
        " Build Output (running...) "
    } else {
        " Build Output "
    };

    let block = Block::default()
        .title(status)
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

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

    // --- Diagnostic summary section ---
    if has_diagnostics {
        render_diagnostic_summary(frame, app, chunks[1]);
    }
}

/// Render a summary of parsed diagnostics at the bottom of the build view.
fn render_diagnostic_summary(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let error_count = app
        .build_diagnostics
        .iter()
        .filter(|d| d.severity == cabalist_cabal::GhcSeverity::Error)
        .count();
    let warning_count = app
        .build_diagnostics
        .iter()
        .filter(|d| d.severity == cabalist_cabal::GhcSeverity::Warning)
        .count();

    let title = format!(
        " Diagnostics: {} error{}, {} warning{} ",
        error_count,
        if error_count == 1 { "" } else { "s" },
        warning_count,
        if warning_count == 1 { "" } else { "s" },
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    if app.build_diagnostics.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No diagnostics.",
            theme.muted_style(),
        )));
    } else {
        let diag = &app.build_diagnostics[app.selected_diagnostic];
        let severity_str = match diag.severity {
            cabalist_cabal::GhcSeverity::Error => "error",
            cabalist_cabal::GhcSeverity::Warning => "warning",
        };
        let severity_style = match diag.severity {
            cabalist_cabal::GhcSeverity::Error => theme.error_style(),
            cabalist_cabal::GhcSeverity::Warning => theme.warning_style(),
        };

        let current = app.selected_diagnostic + 1;
        let total = app.build_diagnostics.len();

        lines.push(Line::from(vec![
            Span::styled(format!("  [{current}/{total}] "), theme.accent()),
            Span::styled(format!("{severity_str}: "), severity_style),
            Span::styled(
                format!("{}:{}:{}", diag.file, diag.line, diag.column),
                theme.bold(),
            ),
        ]));

        // Show a truncated message on the second line.
        let msg = if diag.message.len() > 80 {
            format!("  {}...", &diag.message[..77])
        } else {
            format!("  {}", diag.message)
        };
        lines.push(Line::from(Span::styled(msg, theme.normal())));

        lines.push(Line::from(Span::styled(
            "  [n/]] next  [p/[] prev",
            theme.muted_style(),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
