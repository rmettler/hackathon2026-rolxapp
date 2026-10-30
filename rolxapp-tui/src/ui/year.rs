use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Row, Table};
use rolx_core::api::fmt_duration;

use crate::app::App;

pub(super) fn draw_year(frame: &mut Frame, area: Rect, app: &App) {
    let title = format!(" {} ", app.year);
    let Some(info) = &app.year_info else {
        frame.render_widget(
            Paragraph::new("Loading…").block(Block::bordered().title(title)),
            area,
        );
        return;
    };
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

    let holidays = info
        .holidays
        .iter()
        .map(|h| ListItem::new(format!("{}  {}", h.date, h.name)));
    frame.render_stateful_widget(
        List::new(holidays).block(Block::bordered().title(format!("{title}Holidays "))),
        left,
        &mut ListState::default().with_offset(app.year_scroll),
    );

    let months = info.monthly_work_times.iter().map(|m| {
        Row::new([
            m.month.clone(),
            m.days.to_string(),
            fmt_duration(m.work_time()),
        ])
    });
    frame.render_widget(
        Table::new(
            months,
            [
                Constraint::Length(9),
                Constraint::Length(6),
                Constraint::Fill(1),
            ],
        )
        .header(Row::new(["Month", "Days", "Work time"]).bold())
        .block(Block::bordered().title(" Monthly work time ")),
        right,
    );
}
