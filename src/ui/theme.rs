use ratatui::style::{Color, Modifier, Style};

use crate::diff::model::LineType;

pub struct Theme {
    pub addition: Style,
    pub addition_bg: Style,
    pub deletion: Style,
    pub deletion_bg: Style,
    pub context: Style,
    pub line_number: Style,
    pub line_number_active: Style,
    pub hunk_header: Style,
    pub selection: Style,
    pub comment_indicator: Style,
    pub status_bar: Style,
    pub mode_normal: Style,
    pub mode_visual: Style,
    pub mode_comment: Style,
    pub file_header: Style,
    pub file_list_active: Style,
    pub file_list_item: Style,
    pub border: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            addition: Style::default().fg(Color::Green).bg(Color::Rgb(0, 35, 0)),
            addition_bg: Style::default().bg(Color::Rgb(0, 35, 0)),
            deletion: Style::default().fg(Color::Red).bg(Color::Rgb(40, 0, 0)),
            deletion_bg: Style::default().bg(Color::Rgb(40, 0, 0)),
            context: Style::default().fg(Color::DarkGray),
            line_number: Style::default().fg(Color::DarkGray),
            line_number_active: Style::default().fg(Color::Yellow),
            hunk_header: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            selection: Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            comment_indicator: Style::default().fg(Color::Yellow),
            status_bar: Style::default().bg(Color::DarkGray).fg(Color::White),
            mode_normal: Style::default()
                .bg(Color::Blue)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            mode_visual: Style::default()
                .bg(Color::Magenta)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            mode_comment: Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
            file_header: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            file_list_active: Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            file_list_item: Style::default().fg(Color::Gray),
            border: Style::default().fg(Color::DarkGray),
        }
    }
}

impl Theme {
    pub fn line_style(&self, kind: &LineType) -> Style {
        match kind {
            LineType::Addition => self.addition,
            LineType::Deletion => self.deletion,
            LineType::Context => self.context,
        }
    }

    pub fn mode_style(&self, mode: &crate::vim::mode::Mode) -> Style {
        match mode {
            crate::vim::mode::Mode::Normal => self.mode_normal,
            crate::vim::mode::Mode::VisualLine { .. }
            | crate::vim::mode::Mode::VisualBlock { .. } => self.mode_visual,
            crate::vim::mode::Mode::Comment => self.mode_comment,
            crate::vim::mode::Mode::Command => self.mode_normal,
        }
    }
}
