//! Color palette and styled span helpers.

use ratatui::style::{Color, Modifier, Style};

/// The TUI color theme.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub accent_fg: Color,
    pub warning: Color,
    pub error: Color,
    pub success: Color,
    pub muted: Color,
    pub selected_bg: Color,
    pub selected_fg: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub border: Color,
}

impl Theme {
    /// Dark theme suitable for dark terminal backgrounds.
    pub fn dark() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::White,
            accent: Color::Cyan,
            accent_fg: Color::Black,
            warning: Color::Yellow,
            error: Color::Red,
            success: Color::Green,
            muted: Color::DarkGray,
            selected_bg: Color::Cyan,
            selected_fg: Color::Black,
            header_bg: Color::DarkGray,
            header_fg: Color::White,
            border: Color::DarkGray,
        }
    }

    /// Light theme suitable for light terminal backgrounds.
    pub fn light() -> Self {
        Self {
            bg: Color::Reset,
            fg: Color::Black,
            accent: Color::Blue,
            accent_fg: Color::White,
            warning: Color::Yellow,
            error: Color::Red,
            success: Color::Green,
            muted: Color::Gray,
            selected_bg: Color::Blue,
            selected_fg: Color::White,
            header_bg: Color::Gray,
            header_fg: Color::Black,
            border: Color::Gray,
        }
    }

    pub fn normal(&self) -> Style {
        Style::default().fg(self.fg)
    }

    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn selected(&self) -> Style {
        Style::default().bg(self.selected_bg).fg(self.selected_fg)
    }

    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted)
    }

    pub fn header(&self) -> Style {
        Style::default().bg(self.header_bg).fg(self.header_fg)
    }

    pub fn bold(&self) -> Style {
        Style::default().fg(self.fg).add_modifier(Modifier::BOLD)
    }

    pub fn label_style(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }
}
