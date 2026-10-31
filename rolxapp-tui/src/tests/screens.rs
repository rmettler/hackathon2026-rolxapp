use chrono::{Duration, NaiveDate};
use ratatui::style::Color;
use rolx_core::api::balance::Balance;
use tokio::sync::mpsc;

use crate::app::{App, DayView, Screen};
use crate::test_support::*;

#[test]
fn brand_colours_mark_the_selection_and_table_header() {
    let buffer = render_buffer(&main_app());
    let has_bg = |color: Color| buffer.content().iter().any(|cell| cell.bg == color);
    assert!(has_bg(Color::Rgb(0, 0, 255)), "selected row is brand blue");
    assert!(has_bg(Color::Rgb(220, 220, 212)), "header is brand grey");
}

#[test]
fn the_day_tab_starts_in_the_week_view() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let app = App::new(tx);

    assert!(app.day_view == DayView::Week);
}

#[test]
fn day_tab_shows_entries_and_total() {
    let out = render(&main_app());
    assert!(out.contains("P1-A1"));
    assert!(out.contains("Code review"));
    assert!(out.contains("review"));
    assert!(out.contains("Total 1:30 / target 8:00"));
}

#[test]
fn locked_day_is_marked() {
    let mut app = main_app();
    app.edit_lock = NaiveDate::from_ymd_opt(2026, 11, 1);
    assert!(render(&app).contains("[locked]"));
}

#[test]
fn login_screen_offers_login() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut app = App::new(tx);
    app.screen = Screen::Login {
        busy: false,
        error: None,
    };
    assert!(render(&app).contains("not logged in"));
}

fn vacation_header(
    left: Option<f64>,
    planned: Option<f64>,
    left_hours: i64,
    planned_hours: i64,
) -> String {
    let mut app = main_app();
    app.balance = Some(Balance {
        by_date: "2026-10-30".into(),
        overtime: Duration::zero(),
        vacation_available: Duration::hours(left_hours),
        vacation_available_days: left,
        vacation_planned: Duration::hours(planned_hours),
        vacation_planned_days: planned,
    });
    render(&app)
}

#[test]
fn the_header_shows_the_vacation_left_and_how_much_of_it_is_planned() {
    let out = vacation_header(Some(13.0), Some(8.0), 109, 67);

    assert!(out.contains("Vacation 13 d (8 planned)"));
}

#[test]
fn fractional_vacation_days_get_one_decimal() {
    let out = vacation_header(Some(12.5), Some(8.04), 105, 67);

    assert!(out.contains("Vacation 12.5 d (8 planned)"));
}

#[test]
fn nothing_planned_shows_only_the_vacation_left() {
    let out = vacation_header(Some(13.0), Some(0.0), 109, 0);

    assert!(out.contains("Vacation 13 d"));
    assert!(!out.contains("planned"));
}

#[test]
fn without_a_days_value_the_vacation_is_shown_in_hours() {
    let out = vacation_header(None, None, 104, 64);

    assert!(out.contains("Vacation 104:00 (64:00 planned)"));
}
