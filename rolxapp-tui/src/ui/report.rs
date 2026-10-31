use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use rolx_core::api::fmt_duration;

use super::theme;
use crate::app::App;

pub(super) fn draw_report(frame: &mut Frame, area: Rect, app: &App) {
    let block = theme::panel(format!(
        " Report {}{} ",
        app.month_key(),
        super::as_of(app.report_at)
    ));
    let Some(r) = &app.report else {
        frame.render_widget(Paragraph::new("Loading…").block(block), area);
        return;
    };
    let days = |d: Option<f64>| d.map_or_else(String::new, |d| format!(" ({d:.1} d)"));
    let mut lines = vec![
        Line::from(format!(
            "Overtime       {}  ({:+} this month)",
            fmt_duration(r.overtime),
            r.overtime_delta.num_minutes() as f64 / 60.0
        )),
        Line::from(format!(
            "Vacation       {}{}",
            fmt_duration(r.vacation),
            days(r.vacation_days)
        )),
        Line::from(format!(
            "Vacation delta {}{}",
            fmt_duration(r.vacation_delta),
            days(r.vacation_delta_days)
        )),
        Line::from(format!("Billable       {}", fmt_duration(r.billable_time))),
        Line::from(format!(
            "Non-billable   {}",
            fmt_duration(r.non_billable_time)
        )),
        Line::from(format!("Leave          {}", fmt_duration(r.leave_time))),
    ];
    for group in r.work_item_groups.iter().flatten() {
        lines.push(Line::default());
        lines.push(Line::from(group.name.clone().bold()));
        for item in &group.items {
            lines.push(Line::from(format!(
                "  {:<40} {}",
                item.name,
                fmt_duration(item.duration)
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .scroll((app.report_scroll as u16, 0)),
        area,
    );
}
