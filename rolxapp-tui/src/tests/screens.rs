use chrono::NaiveDate;
use tokio::sync::mpsc;

use crate::app::{App, Screen};
use crate::test_support::*;

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
