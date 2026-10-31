mod absence;
mod activities;
mod charts;
mod day;
mod edit;
mod footer;
mod outbox;
mod report;
pub(crate) mod theme;
mod week;
mod year;

use chrono::{DateTime, Local};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs, Wrap};
use rolx_core::api::balance::Balance;
use rolx_core::api::fmt_duration;

use self::absence::{absence_area, draw_absence};
use self::activities::draw_activities;
use self::charts::{chart_panel, draw_charts};
use self::day::draw_day;
use self::edit::{draw_edit, edit_area};
use self::footer::help_text;
use self::outbox::{conflict_area, draw_conflict, draw_outbox, outbox_area};
use self::report::draw_report;
use self::week::draw_week;
use self::year::draw_year;
use crate::app::{App, DayView, Screen, Tab};
use crate::outbox::ChangeState;
use crate::time::fmt_hms;

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

fn main_layout(area: Rect) -> [Rect; 4] {
    Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .areas(area)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Edit,
    Absence,
    Outbox,
    Conflict,
}

/// The popup drawn on top of the Day tab.
pub struct Overlay {
    pub kind: OverlayKind,
    pub area: Rect,
}

/// The topmost popup of a frame of size `area`, if any.
pub fn overlay(app: &App, area: Rect) -> Option<Overlay> {
    if !matches!(app.screen, Screen::Main) {
        return None;
    }
    let [_, _, body, _] = main_layout(area);
    let (kind, area) = if app.conflict.is_some() {
        (OverlayKind::Conflict, conflict_area(body))
    } else if app.outbox_popup.is_some() {
        (OverlayKind::Outbox, outbox_area(body, app.outbox.len()))
    } else if app.absence_menu.is_some() {
        (OverlayKind::Absence, absence_area(body))
    } else if let Some(form) = &app.edit {
        (OverlayKind::Edit, edit_area(body, form))
    } else {
        return None;
    };
    Some(Overlay { kind, area })
}

/// The line chart's panel while the Charts tab shows one.
pub fn chart_area(app: &App, area: Rect) -> Option<Rect> {
    if !matches!(app.screen, Screen::Main) || app.tab != Tab::Charts {
        return None;
    }
    let [_, _, body, _] = main_layout(area);
    let has_time = app.month_chart().is_some_and(|c| c.has_time());
    has_time.then(|| chart_panel(body))
}

fn draw_main(frame: &mut Frame, area: Rect, app: &App) {
    let [header, tabs, body, footer] = main_layout(area);

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
        Tab::Day => match app.day_view {
            DayView::Day => draw_day(frame, body, app),
            DayView::Week => draw_week(frame, body, app),
        },
        Tab::Activities => draw_activities(frame, body, app),
        Tab::Year => draw_year(frame, body, app),
        Tab::Report => draw_report(frame, body, app),
        Tab::Charts => draw_charts(frame, body, app),
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
    if let Some(selected) = app.absence_menu {
        draw_absence(frame, body, app, selected);
    }
    if let Some(popup) = &app.outbox_popup {
        draw_outbox(frame, body, app, popup);
    }
    if let Some(dialog) = &app.conflict {
        draw_conflict(frame, body, app, dialog);
    }
}

/// " (as of 31.10. 14:05)" for numbers that come from the local copy, else nothing.
fn as_of(at: Option<DateTime<Local>>) -> String {
    at.map_or_else(String::new, |at| {
        format!(" (as of {})", at.format("%d.%m. %H:%M"))
    })
}

/// The connection and upload state, shown only when something is off or waiting.
fn sync_state(app: &App) -> Option<Span<'static>> {
    let mut parts = Vec::new();
    if app.needs_login {
        parts.push("log in (L)".to_string());
    } else if !app.online {
        parts.push("offline".to_string());
    }
    let pending = app.outbox.count(ChangeState::Pending);
    if pending > 0 {
        parts.push(format!("{pending} pending"));
    }
    let attention = app.outbox.needs_attention();
    if attention > 0 {
        parts.push(format!("{attention} need attention (O)"));
    }
    // Only a long download is worth a mention; the regular refresh is a few months.
    if let Some((done, total)) = app.download.filter(|(_, total)| *total > 4) {
        parts.push(format!("Downloading {done}/{total}"));
    }
    if parts.is_empty() {
        return None;
    }
    let text = format!(" {} ", parts.join(" · "));
    Some(if attention > 0 || app.needs_login {
        Span::styled(text, theme::selected())
    } else {
        Span::raw(text).yellow()
    })
}

/// Whole days without decimals, others with one.
fn fmt_days(days: f64) -> String {
    let rounded = (days * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

/// Vacation left, and how much of it is planned; hours when the server has no days.
fn vacation_text(balance: &Balance) -> String {
    let (left, planned) = match balance.vacation_available_days {
        Some(days) => (
            format!("{} d", fmt_days(days)),
            balance.vacation_planned_days.map(fmt_days),
        ),
        None => (
            fmt_duration(balance.vacation_available),
            Some(fmt_duration(balance.vacation_planned)),
        ),
    };
    if balance.vacation_planned > chrono::Duration::zero() {
        format!("{left} ({} planned)", planned.unwrap_or_default())
    } else {
        left
    }
}

fn header_line(app: &App) -> Line<'static> {
    let mut spans = vec![Span::raw(app.user.clone()).bold()];
    if let Some(state) = sync_state(app) {
        spans.push(Span::raw("   "));
        spans.push(state);
    }
    if let Some(elapsed) = app.stamp_elapsed(Local::now()) {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            format!(" ● {} ", fmt_hms(elapsed)),
            theme::selected(),
        ));
    }
    if let Some(balance) = &app.balance {
        spans.push(Span::raw(format!(
            "   Overtime {}",
            fmt_duration(balance.overtime)
        )));
        let vacation = vacation_text(balance);
        spans.push(Span::raw(format!("   Vacation {vacation}")));
        if app.balance_at.is_some() {
            spans.push(Span::raw(as_of(app.balance_at)).dim());
        }
    }
    if let Some(lock) = app.edit_lock {
        spans.push(Span::raw(format!("   Edit lock {lock}")));
    }
    Line::from(spans)
}
