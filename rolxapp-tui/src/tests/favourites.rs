use rolx_core::api::activity::Activity;

use super::week::week_app;
use crate::app::App;
use crate::edit::{EditMode, Field};
use crate::test_support::*;

/// A favourite that is not booked in the fixtures (activity 1 is).
fn meetings() -> Activity {
    Activity {
        id: 7,
        number: 7,
        name: "Meetings".into(),
        full_number: "P2-F".into(),
        full_name: "P2-F Meetings".into(),
        ..main_app().suitable[0].clone()
    }
}

fn with_favourites(mut app: App, favourites: Vec<Activity>) -> App {
    app.favourite_activities = favourites;
    app
}

#[test]
fn the_day_view_lists_unbooked_favourites_below_the_entries() {
    let app = with_favourites(main_app(), vec![meetings()]);

    let out = render(&app);

    assert!(out.contains("★ P2-F"));
    assert!(out.contains("Meetings"));
    assert_eq!(app.day_row_count(), 2);
}

#[test]
fn a_booked_favourite_is_not_listed_twice() {
    let booked = Activity {
        id: 1,
        ..meetings()
    };
    let app = with_favourites(main_app(), vec![booked]);

    assert!(app.unbooked_favourites().is_empty());
    assert!(!render(&app).contains('★'));
}

#[test]
fn an_empty_day_still_shows_its_favourites() {
    let mut app = with_favourites(main_app(), vec![meetings()]);
    app.record.as_mut().unwrap().entries.clear();

    let out = render(&app);

    assert!(out.contains("★ P2-F"));
    assert!(!out.contains("No entries"));
}

#[test]
fn editing_a_favourite_row_books_that_activity() {
    let mut app = with_favourites(main_app(), vec![meetings()]);

    // Down onto the favourite row (the fixture has one entry), then edit.
    press(&mut app, "jl");

    let form = app.edit.as_ref().expect("form is open");
    assert_eq!(form.index, None);
    assert_eq!(form.activity_id, 7);
    assert!(form.field == Field::Duration && form.mode == EditMode::Insert);
}

#[test]
fn paging_reaches_the_favourite_rows_and_dd_ignores_them() {
    let mut app = with_favourites(main_app(), vec![meetings()]);

    press(&mut app, "G");
    assert_eq!(app.sel_entry, 1);

    press(&mut app, "dd");
    assert!(app.edit.is_none());
    assert!(app.pending.is_empty());
}

#[test]
fn the_week_grid_has_a_row_for_every_favourite() {
    let app = with_favourites(week_app(), vec![meetings()]);

    let grid = app.week_grid();
    assert_eq!(grid.rows.len(), 3);
    assert!(grid.rows[0].favourite && grid.rows[0].activity_id == 7);
    assert!(render(&app).contains("★ P2-F"));
}

#[test]
fn an_empty_favourite_cell_can_be_booked_from_the_week_grid() {
    let mut app = with_favourites(week_app(), vec![meetings()]);
    app.sel_row = 0;

    press(&mut app, "e");

    let form = app.edit.as_ref().expect("form is open");
    assert_eq!(form.index, None);
    assert_eq!(form.activity_id, 7);
}

#[test]
fn closed_or_not_yet_open_favourites_are_hidden_in_the_day_view() {
    let ended = Activity {
        end_date: Some("2026-10-01".into()),
        ..meetings()
    };
    let ends_today = Activity {
        id: 8,
        full_number: "P3-E".into(),
        end_date: Some("2026-10-30".into()),
        ..meetings()
    };
    let not_open_yet = Activity {
        id: 9,
        full_number: "P4-N".into(),
        start_date: "2026-11-15".into(),
        ..meetings()
    };
    let app = with_favourites(main_app(), vec![ended, ends_today, not_open_yet]);

    assert!(app.unbooked_favourites().is_empty());
    assert!(!render(&app).contains('★'));
}

#[test]
fn a_favourite_closed_before_the_week_has_no_grid_row() {
    let ended = Activity {
        end_date: Some("2026-10-20".into()),
        ..meetings()
    };
    let app = with_favourites(week_app(), vec![ended]);

    assert_eq!(app.week_grid().rows.len(), 2);
    assert!(!render(&app).contains("P2-F"));
}

#[test]
fn a_favourite_closing_mid_week_cannot_be_booked_afterwards() {
    let closing = Activity {
        end_date: Some("2026-10-28".into()),
        ..meetings()
    };
    let mut app = with_favourites(week_app(), vec![closing]);
    app.sel_row = 0;

    // The selected day is Friday 30.10., after the activity closed.
    press(&mut app, "e");

    assert!(app.edit.is_none());
    assert!(
        app.status
            .as_deref()
            .unwrap_or("")
            .contains("can't be booked")
    );
    assert!(
        app.week_grid().rows[0].favourite,
        "still listed for the open days"
    );
}
