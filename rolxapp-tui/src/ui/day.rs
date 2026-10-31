use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Paragraph, Row, Table, TableState};
use rolx_core::api::fmt_duration;

use super::theme;
use crate::app::{App, absence_of, leave_time};
use crate::outbox::changed_flags;
use crate::time::{entry_end, fmt_clock};

pub(super) fn draw_day(frame: &mut Frame, area: Rect, app: &App) {
    let mut title = format!(" {} ", app.day.format("%a %d.%m.%Y"));
    if app.is_locked() {
        title.push_str("[locked] ");
    }
    let change = app.outbox.get(app.day);
    if let Some(change) = change {
        title.push_str(&format!(
            "[{} {}] ",
            change.state.symbol(),
            change.state.label()
        ));
    }
    let Some(record) = &app.record else {
        frame.render_widget(Paragraph::new("Loading…").block(theme::panel(title)), area);
        return;
    };

    let absence = absence_of(record);
    if let Some(absence) = absence {
        title.push_str(&format!("[{}] ", absence.label()));
    }
    let total: chrono::Duration = record.entries.iter().map(|e| e.duration).sum();
    let nominal = chrono::Duration::seconds(record.nominal_work_time);
    let mut totals = format!(
        " Total {} / target {} ",
        fmt_duration(total),
        fmt_duration(nominal)
    );
    if let Some(leave) = leave_time(record) {
        totals = format!("{}/ leave {} ", totals, fmt_duration(leave));
    }
    let block = theme::panel(title).title_bottom(totals);

    let favourites = app.unbooked_favourites();
    if record.entries.is_empty() && favourites.is_empty() {
        let text = match absence {
            Some(absence) => format!("Absent: {}", absence.label()),
            None => "No entries for this day.".into(),
        };
        frame.render_widget(Paragraph::new(text).block(block), area);
        return;
    }

    let added = change.map(|c| changed_flags(&c.base, record));
    let entry_rows = record.entries.iter().enumerate().map(|(i, e)| {
        let mark = if added.as_ref().is_some_and(|flags| flags[i]) {
            "↑ "
        } else {
            ""
        };
        let name = app
            .suitable
            .iter()
            .chain(&app.activities)
            .find(|a| a.id == e.activity_id)
            .map_or("", |a| a.name.as_str());
        Row::new([
            format!("{mark}{}", e.full_activity_number),
            name.to_string(),
            e.begin.map(|b| fmt_clock(b.into())).unwrap_or_default(),
            e.begin
                .map(|b| fmt_clock(entry_end(b, e)))
                .unwrap_or_default(),
            fmt_duration(e.duration),
            e.comment.clone().unwrap_or_default(),
        ])
    });
    let favourite_rows = favourites.iter().map(|a| {
        Row::new([
            format!("★ {}", a.full_number),
            a.name.clone(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ])
        .style(Style::new().add_modifier(Modifier::DIM))
    });
    let rows: Vec<Row> = entry_rows.chain(favourite_rows).collect();
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
    .header(Row::new(["Activity", "Name", "From", "To", "Time", "Comment"]).style(theme::header()))
    .row_highlight_style(theme::selected())
    .block(block);
    let mut state = TableState::default().with_selected(Some(app.sel_entry));
    frame.render_stateful_widget(table, area, &mut state);
}
