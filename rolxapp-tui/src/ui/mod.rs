mod activities;
mod day;
mod edit;
mod footer;
mod report;
mod theme;
mod year;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs, Wrap};
use rolx_core::api::fmt_duration;

use self::activities::draw_activities;
use self::day::draw_day;
use self::edit::draw_edit;
use self::footer::help_text;
use self::report::draw_report;
use self::year::draw_year;
use crate::app::{App, Screen, Tab};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    match &app.screen {
        Screen::Connecting => centered_text(frame, area, "Connecting…", "q quit"),
        Screen::Login { busy: true, .. } => {
            centered_text(frame, area, "Waiting for the browser login…", "q quit");
        }
        Screen::Login { error, .. } => {
            let mut text = String::from("You are not logged in.");
            if let Some(error) = error {
                text.push_str(&format!("\n\n{error}"));
            }
            centered_text(frame, area, &text, "l log in with Google   q quit");
        }
        Screen::Main => draw_main(frame, area, app),
    }
}

fn centered_text(frame: &mut Frame, area: Rect, text: &str, help: &str) {
    let [body, footer] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let lines = text.lines().count() as u16;
    let [_, middle, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(lines),
        Constraint::Fill(1),
    ])
    .areas(body);
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        middle,
    );
    frame.render_widget(Paragraph::new(help).dim(), footer);
}

fn draw_main(frame: &mut Frame, area: Rect, app: &App) {
    let [header, tabs, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(area);

    frame.render_widget(
        Paragraph::new(header_line(app)).block(theme::panel(" RolX ")),
        header,
    );
    frame.render_widget(
        Tabs::new(Tab::ALL.map(Tab::title))
            .select(app.tab.index())
            .highlight_style(theme::selected()),
        tabs,
    );

    match app.tab {
        Tab::Day => draw_day(frame, body, app),
        Tab::Activities => draw_activities(frame, body, app),
        Tab::Year => draw_year(frame, body, app),
        Tab::Report => draw_report(frame, body, app),
    }

    let help = help_text(app);
    let status = if app.pending.is_empty() {
        app.status.as_deref().unwrap_or("")
    } else {
        app.pending.as_str()
    };
    frame.render_widget(
        Paragraph::new(vec![Line::from(status.yellow()), Line::from(help.dim())]),
        footer,
    );

    if let Some(form) = &app.edit {
        draw_edit(frame, body, app, form);
    }
}

fn header_line(app: &App) -> Line<'static> {
    let mut spans = vec![Span::raw(app.user.clone()).bold()];
    if let Some(balance) = &app.balance {
        spans.push(Span::raw(format!(
            "   Overtime {}",
            fmt_duration(balance.overtime)
        )));
        let vacation = balance.vacation_available_days.map_or_else(
            || fmt_duration(balance.vacation_available),
            |d| format!("{d:.1} d"),
        );
        spans.push(Span::raw(format!("   Vacation {vacation}")));
    }
    if let Some(lock) = app.edit_lock {
        spans.push(Span::raw(format!("   Edit lock {lock}")));
    }
    Line::from(spans)
}
