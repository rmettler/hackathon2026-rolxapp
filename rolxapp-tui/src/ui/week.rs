use std::iter::once;

use chrono::{Datelike, Duration, Local};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};
use rolx_core::api::fmt_duration;

use super::theme;
use crate::app::App;
use crate::time::week_start;

pub(super) fn draw_week(frame: &mut Frame, area: Rect, app: &App) {
    let monday = week_start(app.day);
    let mut title = format!(
        " Week {}  {}-{} ",
        app.day.iso_week().week(),
        monday.format("%d.%m."),
        (monday + Duration::days(6)).format("%d.%m.%Y")
    );
    if app.is_locked() {
        title.push_str("[locked] ");
    }
    if app.week.is_empty() {
        frame.render_widget(Paragraph::new("Loading…").block(theme::panel(title)), area);
        return;
    }

    let grid = app.week_grid();
    let today = Local::now().date_naive();
    let right = |text: String| Cell::new(Line::from(text).right_aligned());
    let dim = Style::new().add_modifier(Modifier::DIM);
    let column_style = |col: usize| {
        if grid.day_types[col] == 0 {
            Style::new()
        } else {
            dim
        }
    };

    let header = Row::new(
        once(Cell::new("Activity"))
            .chain((0..7).map(|col| {
                let date = monday + Duration::days(col as i64);
                let style = if date == today {
                    Style::new().add_modifier(Modifier::UNDERLINED)
                } else {
                    Style::new()
                };
                right(date.format("%a %d").to_string()).style(style)
            }))
            .chain(once(right("Sum".into()))),
    )
    .style(theme::header());

    let mut rows: Vec<Row> = grid
        .rows
        .iter()
        .map(|r| {
            let name = app
                .find_activity(r.activity_id)
                .map_or("", |a| a.name.as_str());
            let cells = r
                .cells
                .iter()
                .enumerate()
                .map(|(col, c)| right(c.text()).style(column_style(col)));
            let star = if r.favourite { "★ " } else { "" };
            Row::new(
                once(Cell::new(format!("{star}{} {name}", r.number)))
                    .chain(cells)
                    .chain(once(right(fmt_duration(r.total())))),
            )
        })
        .collect();

    let summary = |label: &str, values: &[Duration; 7], total: Duration, style: Style| {
        Row::new(
            once(Cell::new(label.to_string()))
                .chain(values.iter().map(|d| right(fmt_duration(*d))))
                .chain(once(right(fmt_duration(total)))),
        )
        .style(style)
    };
    let has_absence = grid.absences.iter().any(Option::is_some);
    if has_absence {
        let cells = grid
            .absences
            .iter()
            .map(|a| right(a.map_or(String::new(), |a| a.short().to_string())));
        rows.push(
            Row::new(
                once(Cell::new("Absence"))
                    .chain(cells)
                    .chain(once(right(String::new()))),
            )
            .top_margin(1),
        );
    }
    let marks: Vec<String> = (0..7)
        .map(|col| {
            app.outbox
                .get(monday + Duration::days(col))
                .map_or(String::new(), |c| c.state.symbol().to_string())
        })
        .collect();
    let has_sync = marks.iter().any(|m| !m.is_empty());
    if has_sync {
        let row = Row::new(
            once(Cell::new("Sync"))
                .chain(marks.into_iter().map(right))
                .chain(once(right(String::new()))),
        );
        rows.push(if has_absence { row } else { row.top_margin(1) });
    }
    let has_extra = has_absence || has_sync;
    rows.push({
        let total = summary(
            "Total",
            &grid.totals,
            grid.week_total(),
            Style::new().add_modifier(Modifier::BOLD),
        );
        if has_extra {
            total
        } else {
            total.top_margin(1)
        }
    });
    rows.push(summary("Target", &grid.targets, grid.week_target(), dim));

    let mut widths = vec![Constraint::Fill(1)];
    widths.extend([Constraint::Length(6); 8]);
    let table = Table::new(rows, widths)
        .header(header)
        .block(theme::panel(title))
        .cell_highlight_style(theme::selected());
    let mut state = TableState::default();
    if !grid.rows.is_empty() {
        state.select_cell(Some((app.sel_row, app.sel_col() + 1)));
    }
    frame.render_stateful_widget(table, area, &mut state);
}
