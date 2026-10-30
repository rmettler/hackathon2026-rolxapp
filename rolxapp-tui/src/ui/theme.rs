//! Cudos brand colours from the brand manual (Farben).

use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Block;

pub const BLUE: Color = Color::Rgb(0, 0, 255);
pub const GREY: Color = Color::Rgb(220, 220, 212);
pub const WHITE: Color = Color::Rgb(255, 255, 255);
pub const BLACK: Color = Color::Rgb(0, 0, 0);

/// Selected row, active tab or text cursor: white on brand blue.
pub fn selected() -> Style {
    Style::new().fg(WHITE).bg(BLUE)
}

/// Column headers: black on brand grey.
pub fn header() -> Style {
    Style::new().fg(BLACK).bg(GREY).bold()
}

/// Bordered panel with a brand-blue frame.
pub fn panel<'a>(title: impl Into<Line<'a>>) -> Block<'a> {
    Block::bordered()
        .border_style(Style::new().fg(BLUE))
        .title(title)
}
