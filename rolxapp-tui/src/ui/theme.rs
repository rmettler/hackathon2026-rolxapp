//! Cudos brand colours from the brand manual (Farben).

use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Block;

pub const BLUE: Color = Color::Rgb(0, 0, 255);
pub const GREY: Color = Color::Rgb(220, 220, 212);
pub const WHITE: Color = Color::Rgb(255, 255, 255);
pub const BLACK: Color = Color::Rgb(0, 0, 0);

/// Slice colours of the pie chart, biggest slice first.
pub const PALETTE: [Color; 8] = [
    BLUE,
    Color::Rgb(255, 140, 0),
    Color::Rgb(0, 160, 80),
    Color::Rgb(200, 0, 160),
    Color::Rgb(0, 190, 210),
    Color::Rgb(220, 40, 40),
    Color::Rgb(230, 200, 0),
    Color::Rgb(130, 70, 200),
];

/// The colour of the slice at `index`; the combined "Other" slice and anything after the palette is grey.
pub fn slice_colour(index: usize) -> Color {
    PALETTE.get(index).copied().unwrap_or(Color::Gray)
}

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
