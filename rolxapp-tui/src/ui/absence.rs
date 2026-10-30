use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};

use super::theme;
use crate::app::{Absence, App, absence_of};

pub(super) fn draw_absence(frame: &mut Frame, area: Rect, app: &App, selected: usize) {
    let current = app.record.as_ref().and_then(absence_of);
    let mut lines: Vec<Line> = Absence::ALL
        .iter()
        .enumerate()
        .map(|(i, absence)| {
            let mark = if Some(*absence) == current {
                "✓"
            } else {
                " "
            };
            let line = Line::from(format!(" {} {mark} {}", i + 1, absence.label()));
            if i == selected {
                line.style(theme::selected())
            } else {
                line
            }
        })
        .collect();
    lines.push(Line::default());
    lines.push(Line::from(" Books the rest of the day".dim()));

    let width = area.width.min(34);
    let height = 8.min(area.height);
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let title = format!(" Absence {} ", app.day.format("%a %d.%m."));
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(theme::panel(title)), popup);
}
