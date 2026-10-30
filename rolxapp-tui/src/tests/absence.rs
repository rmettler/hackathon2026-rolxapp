use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::week::week_app;
use crate::app::DayView;
use crate::test_support::*;

#[test]
fn a_opens_the_absence_popup_in_both_views() {
    let mut app = main_app();
    press(&mut app, "a");
    assert_eq!(app.absence_menu, Some(0));
    let out = render(&app);
    for expected in ["Absence", "Krank", "Ferien", "Militär"] {
        assert!(out.contains(expected), "missing {expected:?}");
    }

    let mut app = week_app();
    assert!(app.day_view == DayView::Week);
    press(&mut app, "a");
    assert_eq!(app.absence_menu, Some(0));
}

#[test]
fn the_popup_moves_cancels_and_picks() {
    let mut app = main_app();
    press(&mut app, "a");
    press(&mut app, "jj");
    assert_eq!(app.absence_menu, Some(2));
    press(&mut app, "j");
    assert_eq!(app.absence_menu, Some(2), "stops at the last option");
    press(&mut app, "k");
    assert_eq!(app.absence_menu, Some(1));

    special(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.absence_menu, None);

    press(&mut app, "a");
    press(&mut app, "2");
    assert_eq!(app.absence_menu, None, "picking closes the popup");
}

#[test]
fn the_popup_starts_on_the_current_absence() {
    let mut app = main_app();
    app.record.as_mut().unwrap().paid_leave_type = Some(2);

    press(&mut app, "a");

    assert_eq!(app.absence_menu, Some(2));
    assert!(render(&app).contains("✓"));
}

#[test]
fn weekends_and_locked_days_do_not_open_the_popup() {
    let mut app = main_app();
    app.record.as_mut().unwrap().day_type = 1;
    press(&mut app, "a");
    assert_eq!(app.absence_menu, None);
    assert!(app.status.as_deref().unwrap_or("").contains("Weekend"));

    let mut app = main_app();
    app.edit_lock = chrono::NaiveDate::from_ymd_opt(2026, 11, 1);
    press(&mut app, "a");
    assert_eq!(app.absence_menu, None);
    assert!(app.status.as_deref().unwrap_or("").contains("read-only"));
}

#[test]
fn the_day_view_shows_the_absence_and_leave_time() {
    let mut app = main_app();
    app.record.as_mut().unwrap().paid_leave_type = Some(1);

    let out = render(&app);

    assert!(out.contains("[Krank]"));
    assert!(out.contains("leave 6:30"));
}

#[test]
fn the_week_grid_shows_an_absence_row() {
    let mut app = week_app();
    app.week[1].paid_leave_type = Some(0);
    app.sync_week_record();

    let out = render(&app);

    assert!(out.contains("Absence"));
    assert!(out.contains("Ferien"));
}
