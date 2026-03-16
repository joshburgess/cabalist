//! Init wizard view — step-by-step project creation.

use crate::app::{App, InitStep, InitWizard};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Render the init wizard view.
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let wizard = match app.init_wizard.as_ref() {
        Some(w) => w,
        None => return,
    };
    let theme = &app.theme;

    let block = Block::default()
        .title(" Init Wizard — Create New Project ")
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // progress indicator
            Constraint::Min(0),    // step content
            Constraint::Length(2), // navigation hints
        ])
        .split(inner);

    render_progress(frame, app, wizard, chunks[0]);
    render_step(frame, app, wizard, chunks[1]);
    render_hints(frame, app, wizard, chunks[2]);
}

fn render_progress(frame: &mut Frame, app: &App, wizard: &InitWizard, area: Rect) {
    let theme = &app.theme;
    let step_num = wizard.step.number();
    let total = 6;

    let steps = [
        "Name", "Template", "License", "Author", "Synopsis", "Confirm",
    ];
    let mut spans = Vec::new();
    spans.push(Span::styled("  ", theme.normal()));
    for (i, name) in steps.iter().enumerate() {
        let n = i + 1;
        if n == step_num {
            spans.push(Span::styled(
                format!("[{n}:{name}]"),
                ratatui::style::Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        } else if n < step_num {
            spans.push(Span::styled(format!("{n}:{name}"), theme.success_style()));
        } else {
            spans.push(Span::styled(format!("{n}:{name}"), theme.muted_style()));
        }
        if n < total {
            spans.push(Span::styled(" > ", theme.muted_style()));
        }
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(vec![line]), area);
}

fn render_step(frame: &mut Frame, app: &App, wizard: &InitWizard, area: Rect) {
    let theme = &app.theme;

    let lines = match wizard.step {
        InitStep::Name => {
            let value = if wizard.editing {
                &wizard.input_buffer
            } else {
                &wizard.name
            };
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Project Name",
                    ratatui::style::Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  The name of your Haskell package. This will be used for the",
                    theme.normal(),
                )),
                Line::from(Span::styled(
                    "  .cabal file name and directory structure.",
                    theme.normal(),
                )),
                Line::from(""),
                input_line(theme, "Name", value, wizard.editing),
            ]
        }
        InitStep::Template => {
            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Project Type",
                    ratatui::style::Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Choose the project template. Press Tab to cycle options.",
                    theme.normal(),
                )),
                Line::from(""),
            ];
            for kind in cabalist_opinions::templates::TemplateKind::all() {
                let marker = if *kind == wizard.template {
                    " > "
                } else {
                    "   "
                };
                let style = if *kind == wizard.template {
                    ratatui::style::Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    theme.normal()
                };
                lines.push(Line::from(Span::styled(
                    format!("  {marker}{}", kind.label()),
                    style,
                )));
            }
            lines
        }
        InitStep::License => {
            let value = if wizard.editing {
                &wizard.input_buffer
            } else {
                &wizard.license
            };
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  License",
                    ratatui::style::Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  The SPDX license identifier for your package.",
                    theme.normal(),
                )),
                Line::from(Span::styled(
                    "  Common choices: MIT, BSD-3-Clause, Apache-2.0, MPL-2.0",
                    theme.muted_style(),
                )),
                Line::from(""),
                input_line(theme, "License", value, wizard.editing),
            ]
        }
        InitStep::Author => {
            let value = if wizard.editing {
                &wizard.input_buffer
            } else {
                &wizard.author
            };
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Author / Maintainer",
                    ratatui::style::Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Your name and email for the author and maintainer fields.",
                    theme.normal(),
                )),
                Line::from(""),
                input_line(theme, "Author", value, wizard.editing),
                Line::from(""),
                Line::from(vec![
                    Span::styled("    Maintainer: ", theme.muted_style()),
                    Span::styled(&wizard.maintainer, theme.normal()),
                ]),
            ]
        }
        InitStep::Synopsis => {
            let value = if wizard.editing {
                &wizard.input_buffer
            } else {
                &wizard.synopsis
            };
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Synopsis",
                    ratatui::style::Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  A short, one-line description of your package.",
                    theme.normal(),
                )),
                Line::from(""),
                input_line(theme, "Synopsis", value, wizard.editing),
            ]
        }
        InitStep::Confirm => {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Review Settings",
                    ratatui::style::Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                summary_line(theme, "Name", &wizard.name),
                summary_line(theme, "Template", wizard.template.label()),
                summary_line(theme, "License", &wizard.license),
                summary_line(theme, "Author", &wizard.author),
                summary_line(theme, "Maintainer", &wizard.maintainer),
                summary_line(theme, "Synopsis", &wizard.synopsis),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press Enter to create the project.",
                    theme.success_style(),
                )),
            ]
        }
    };

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn input_line<'a>(
    theme: &crate::theme::Theme,
    label: &'a str,
    value: &'a str,
    editing: bool,
) -> Line<'a> {
    let cursor = if editing { "_" } else { "" };
    Line::from(vec![
        Span::styled(format!("    {label}: "), theme.bold()),
        Span::styled(value.to_string(), theme.accent()),
        Span::styled(cursor.to_string(), theme.accent()),
    ])
}

fn summary_line<'a>(theme: &crate::theme::Theme, label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("    {label:<14}"), theme.muted_style()),
        Span::styled(value.to_string(), theme.normal()),
    ])
}

fn render_hints(frame: &mut Frame, app: &App, wizard: &InitWizard, area: Rect) {
    let theme = &app.theme;

    let hints = match wizard.step {
        InitStep::Template => " [Tab] cycle option  [Enter] next  [Esc] back",
        InitStep::Confirm => " [Enter] create project  [Esc] back",
        _ => " [Enter] next  [Esc] back",
    };

    let line = Line::from(Span::styled(hints, theme.muted_style()));
    frame.render_widget(Paragraph::new(vec![line]), area);
}
