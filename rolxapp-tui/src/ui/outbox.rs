use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph};
use rolx_core::api::fmt_duration;
use rolx_core::api::records::{Record, RecordEntry};

use super::theme;
use crate::app::{App, ConflictDialog, OutboxPopup, absence_of};
use crate::outbox::ChangeState;
use crate::time::{entry_end, fmt_clock};

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub(super) fn outbox_area(area: Rect, changes: usize) -> Rect {
    centered(area, 76, changes as u16 + 2)
}

pub(super) fn conflict_area(area: Rect) -> Rect {
    centered(area, 100, 18)
}

pub(super) fn draw_outbox(frame: &mut Frame, area: Rect, app: &App, popup: &OutboxPopup) {
    let lines: Vec<Line> = app
        .outbox
        .by_date()
        .iter()
        .enumerate()
        .map(|(i, change)| {
            let mut text = format!(
                " {} {}  {}",
                change.state.symbol(),
                change.date.format("%a %d.%m.%Y"),
                change.state.label()
            );
            match (change.state, &change.message) {
                (ChangeState::Rejected, Some(message)) => text.push_str(&format!(": {message}")),
                (ChangeState::Conflict, _) => text.push_str(": changed on the server too"),
                _ => {}
            }
            let line = Line::from(text);
            if i == popup.selected {
                line.style(theme::selected())
            } else {
                line
            }
        })
        .collect();
    let popup_area = outbox_area(area, lines.len());
    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Paragraph::new(lines).block(
            theme::panel(" Pending changes ")
                .title_bottom(" Enter open  d discard  r retry  Esc close "),
        ),
        popup_area,
    );
}

fn entry_text(entry: &RecordEntry) -> String {
    let times = entry.begin.map_or(String::new(), |begin| {
        format!(
            "{}-{} ",
            fmt_clock(begin.into()),
            fmt_clock(entry_end(begin, entry))
        )
    });
    let comment = entry.comment.as_deref().unwrap_or("");
    format!(
        "{} {times}{} {comment}",
        entry.full_activity_number,
        fmt_duration(entry.duration)
    )
}

fn day_lines(record: &Record) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let Some(absence) = absence_of(record) {
        lines.push(Line::from(format!("Absent: {}", absence.label())));
    }
    lines.extend(
        record
            .entries
            .iter()
            .filter(|e| e.duration.num_seconds() > 0)
            .map(|e| Line::from(entry_text(e))),
    );
    if lines.is_empty() {
        lines.push(Line::from("No entries".dim()));
    }
    lines
}

pub(super) fn draw_conflict(frame: &mut Frame, area: Rect, app: &App, dialog: &ConflictDialog) {
    let Some(change) = app.outbox.get(dialog.date) else {
        return;
    };
    let popup = conflict_area(area);
    frame.render_widget(Clear, popup);
    let title = format!(" Conflict {} ", dialog.date.format("%a %d.%m.%Y"));
    let block = theme::panel(title).title_bottom(" j/k or 1-3 choose  Enter apply  Esc later ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [note, columns, choices] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(4),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new("This day was changed on the server after your change.")
            .wrap(ratatui::widgets::Wrap { trim: true }),
        note,
    );

    let [left, right] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(columns);
    frame.render_widget(
        Paragraph::new(day_lines(&change.record)).block(theme::panel(" Yours ")),
        left,
    );
    let server = change.server.as_ref().map(day_lines).unwrap_or_default();
    frame.render_widget(
        Paragraph::new(server).block(theme::panel(" On the server ")),
        right,
    );

    let labels = [
        match &dialog.merged {
            Ok(_) => "1 Merge: your changes on top of the server's day".to_string(),
            Err(reason) => format!("1 Merge not possible: {reason}"),
        },
        "2 Keep mine: replace the server's day".to_string(),
        "3 Keep theirs: drop your change".to_string(),
    ];
    let lines: Vec<Line> = labels
        .into_iter()
        .enumerate()
        .map(|(i, label)| {
            let line = Line::from(format!(" {label}"));
            if i == dialog.choice {
                line.style(theme::selected())
            } else if i == 0 && dialog.merged.is_err() {
                line.dim()
            } else {
                line
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), choices);
}
