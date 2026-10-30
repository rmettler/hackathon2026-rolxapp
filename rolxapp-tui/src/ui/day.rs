use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Block, Paragraph, Row, Table, TableState};
use rolx_core::api::fmt_duration;

use crate::app::App;
use crate::time::{entry_end, fmt_clock};

pub(super) fn draw_day(frame: &mut Frame, area: Rect, app: &App) {
    let mut title = format!(" {} ", app.day.format("%a %d.%m.%Y"));
    if app.is_locked() {
        title.push_str("[locked] ");
    }
    let Some(record) = &app.record else {
        frame.render_widget(
            Paragraph::new("Loading…").block(Block::bordered().title(title)),
            area,
        );
        return;
    };

    let total: chrono::Duration = record.entries.iter().map(|e| e.duration).sum();
    let nominal = chrono::Duration::seconds(record.nominal_work_time);
    let block = Block::bordered().title(title).title_bottom(format!(
        " Total {} / target {} ",
        fmt_duration(total),
        fmt_duration(nominal)
    ));

    if record.entries.is_empty() {
        frame.render_widget(
            Paragraph::new("No entries for this day.").block(block),
            area,
        );
        return;
    }

    let rows = record.entries.iter().map(|e| {
        let name = app
            .suitable
            .iter()
            .chain(&app.activities)
            .find(|a| a.id == e.activity_id)
            .map_or("", |a| a.name.as_str());
        Row::new([
            e.full_activity_number.clone(),
            name.to_string(),
            e.begin.map(|b| fmt_clock(b.into())).unwrap_or_default(),
            e.begin
                .map(|b| fmt_clock(entry_end(b, e)))
                .unwrap_or_default(),
            fmt_duration(e.duration),
            e.comment.clone().unwrap_or_default(),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Fill(1),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Fill(1),
        ],
    )
    .header(Row::new(["Activity", "Name", "From", "To", "Time", "Comment"]).bold())
    .row_highlight_style(Style::new().reversed())
    .block(block);
    let mut state = TableState::default().with_selected(Some(app.sel_entry));
    frame.render_stateful_widget(table, area, &mut state);
}
