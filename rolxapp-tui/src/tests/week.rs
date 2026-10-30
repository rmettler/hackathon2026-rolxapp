use chrono::{Duration, NaiveDate};
use ratatui::style::Color;
use rolx_core::api::records::{Record, RecordEntry};

use crate::app::{App, DayView};
use crate::edit::{EditMode, Field};
use crate::test_support::*;

/// Friday 2026-10-30 books P1-A1 (activity 1); Monday books P1-B (activity 2).
pub(super) fn week_app() -> App {
    let mut app = main_app();
    let friday = app.record.clone().unwrap();
    let day = |date: &str, entries: Vec<RecordEntry>, day_type: i32| Record {
        date: date.into(),
        day_type,
        entries,
        ..friday.clone()
    };
    let monday_entry = RecordEntry {
        activity_id: 2,
        full_activity_number: "P1-B".into(),
        duration: Duration::minutes(60),
        comment: None,
        ..friday.entries[0].clone()
    };
    app.week = vec![
        day("2026-10-26", vec![monday_entry], 0),
        day("2026-10-27", vec![], 0),
        day("2026-10-28", vec![], 0),
        day("2026-10-29", vec![], 0),
        day("2026-10-30", friday.entries.clone(), 0),
        day("2026-10-31", vec![], 1),
        day("2026-11-01", vec![], 1),
    ];
    app.day_view = DayView::Week;
    app.sync_week_record();
    app
}

#[test]
fn w_toggles_between_day_and_week() {
    let mut app = main_app();
    press(&mut app, "w");
    assert!(app.day_view == DayView::Week);
    press(&mut app, "w");
    assert!(app.day_view == DayView::Day);
}

#[test]
fn week_view_shows_days_activities_and_totals() {
    let out = render(&week_app());
    for expected in [
        "Week 44", "Mon 26", "Fri 30", "P1-A1", "P1-B", "1:30", "Total", "Target",
    ] {
        assert!(out.contains(expected), "missing {expected:?}");
    }
}

#[test]
fn selected_cell_is_brand_blue() {
    let buffer = render_buffer(&week_app());
    assert!(
        buffer
            .content()
            .iter()
            .any(|c| c.bg == Color::Rgb(0, 0, 255))
    );
}

#[test]
fn h_l_move_days_and_j_k_move_activities() {
    let mut app = week_app();
    assert_eq!(app.sel_col(), 4);

    press(&mut app, "h");
    assert_eq!(app.day, NaiveDate::from_ymd_opt(2026, 10, 29).unwrap());
    assert!(app.record.as_ref().unwrap().entries.is_empty());
    press(&mut app, "l");
    assert_eq!(app.record.as_ref().unwrap().entries.len(), 1);

    press(&mut app, "j");
    assert_eq!(app.sel_row, 1);
    press(&mut app, "j");
    assert_eq!(app.sel_row, 1, "stops at the last activity");
    press(&mut app, "k");
    assert_eq!(app.sel_row, 0);
}

#[test]
fn bracket_keys_change_the_week() {
    let mut app = week_app();
    press(&mut app, "[");
    assert_eq!(app.day, NaiveDate::from_ymd_opt(2026, 10, 23).unwrap());
    assert!(app.week.is_empty(), "the new week is reloaded");
    assert!(app.record.is_none());
}

#[test]
fn editing_a_filled_cell_opens_that_entry() {
    let mut app = week_app();
    press(&mut app, "e");

    let form = app.edit.as_ref().expect("form is open");
    assert_eq!(form.index, Some(0));
    assert_eq!(form.comment, "review");
}

#[test]
fn editing_an_empty_cell_books_the_rows_activity() {
    let mut app = week_app();
    // Back to Monday, whose P1-A1 cell is empty.
    press(&mut app, "hhhh");
    press(&mut app, "e");

    let form = app.edit.as_ref().expect("form is open");
    assert_eq!(form.index, None);
    assert_eq!(form.activity_id, 1);
    assert!(form.field == Field::Duration && form.mode == EditMode::Insert);
}

#[test]
fn a_cell_with_several_entries_opens_the_day_view() {
    let mut app = week_app();
    let extra = app.week[4].entries[0].clone();
    app.week[4].entries.push(extra);
    app.sync_week_record();

    press(&mut app, "e");

    assert!(app.day_view == DayView::Day);
    assert!(app.edit.is_none());
    assert!(app.status.as_deref().unwrap_or("").contains("Several"));
}

#[test]
fn locked_days_cannot_be_edited_from_the_week() {
    let mut app = week_app();
    app.edit_lock = NaiveDate::from_ymd_opt(2026, 11, 1);

    press(&mut app, "e");

    assert!(app.edit.is_none());
    assert!(app.status.as_deref().unwrap_or("").contains("read-only"));
    assert!(render(&app).contains("[locked]"));
}
