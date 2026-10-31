use std::fs;

use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, Msg, StampPart};
use crate::edit::{EditMode, Field};
use crate::stamp_store::StampStore;
use crate::test_support::*;

fn at(h: u32, m: u32, s: u32) -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 30, h, m, s).unwrap()
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
}

fn part(d: u32, begin: i32, end: i32) -> StampPart {
    StampPart {
        date: day(d),
        begin,
        end,
    }
}

/// The fixture's record for the given October day.
fn record_of(app: &App, d: u32) -> rolx_core::api::records::Record {
    let mut record = app.record.clone().unwrap();
    record.date = format!("2026-10-{d:02}");
    record
}

fn stamped_app() -> App {
    let mut app = main_app();
    app.stamp_store = Some(temp_store());
    app
}

/// An app with the stamp-out form open for the given parts.
fn stamping_out(parts: Vec<StampPart>) -> App {
    let mut app = stamped_app();
    app.stamp = Some(at(9, 0, 0));
    let records = parts
        .iter()
        .map(|p| record_of(&app, p.date.format("%d").to_string().parse().unwrap()))
        .collect();
    app.update(Msg::StampDays { parts, records });
    app
}

#[test]
fn s_stamps_in_and_writes_the_file() {
    let mut app = stamped_app();

    press(&mut app, "s");

    assert!(app.stamp.is_some());
    let saved = app.stamp_store.as_ref().unwrap().load().unwrap();
    assert_eq!(saved, app.stamp);
    assert!(
        render(&app).contains('●'),
        "the header shows the running stamp"
    );
}

#[test]
fn a_running_stamp_survives_a_restart() {
    let store = temp_store();
    store.save(at(9, 12, 40)).unwrap();

    let mut app = main_app();
    app.stamp_store = Some(StampStore::at(store.path().to_path_buf()));
    app.load_stamp();

    assert_eq!(app.stamp, Some(at(9, 12, 40)));
}

#[test]
fn an_unreadable_stamp_file_warns_and_is_left_alone() {
    let store = temp_store();
    store.save(at(9, 0, 0)).unwrap();
    fs::write(store.path(), "garbage").unwrap();

    let mut app = main_app();
    app.stamp_store = Some(StampStore::at(store.path().to_path_buf()));
    app.load_stamp();

    assert_eq!(app.stamp, None);
    assert!(app.status.as_deref().unwrap_or("").contains("Can't read"));
    assert_eq!(fs::read_to_string(store.path()).unwrap(), "garbage");
}

#[test]
fn the_elapsed_time_counts_from_the_start() {
    let mut app = main_app();
    app.stamp = Some(at(9, 0, 0));

    assert_eq!(
        app.stamp_elapsed(at(10, 23, 45)),
        Some(Duration::seconds(5025))
    );
}

#[test]
fn stamping_out_opens_the_form_filled_with_the_times() {
    let app = stamping_out(vec![part(30, 36_000, 37_800)]);

    let form = app.edit.as_ref().expect("form is open");
    assert!(form.stamp.is_some());
    assert_eq!((form.from.as_str(), form.to.as_str()), ("10:00", "10:30"));
    assert!(form.field == Field::Activity && form.mode == EditMode::Search);
    assert_eq!(form.activity_id, 1);
    let out = render(&app);
    assert!(out.contains("Stamp out"));
    assert!(out.contains("1 entry"));
}

#[test]
fn the_stamp_form_has_no_duration_field() {
    let mut app = stamping_out(vec![part(30, 36_000, 37_800)]);
    special(&mut app, KeyCode::Esc, KeyModifiers::NONE);

    press(&mut app, "jj");
    assert!(app.edit.as_ref().unwrap().field == Field::To);
    press(&mut app, "j");
    assert!(app.edit.as_ref().unwrap().field == Field::Comment);
    press(&mut app, "k");
    assert!(app.edit.as_ref().unwrap().field == Field::To);
    assert!(!render(&app).contains("Duration"));
}

#[test]
fn cancelling_the_form_keeps_the_stamp_running() {
    let mut app = stamping_out(vec![part(30, 36_000, 37_800)]);
    app.stamp_store.as_ref().unwrap().save(at(9, 0, 0)).unwrap();

    special(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    special(&mut app, KeyCode::Esc, KeyModifiers::NONE);

    assert!(app.edit.is_none());
    assert_eq!(app.stamp, Some(at(9, 0, 0)));
    assert_eq!(
        app.stamp_store.as_ref().unwrap().load().unwrap(),
        Some(at(9, 0, 0))
    );
}

#[test]
fn a_stamp_becomes_one_entry_with_the_chosen_activity_and_comment() {
    let mut app = stamping_out(vec![part(30, 36_000, 37_800)]);
    app.edit.as_mut().unwrap().comment = "review".into();

    let records = app.stamp_records().unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].entries.len(), 2, "the existing entry stays");
    let entry = records[0].entries.last().unwrap();
    assert_eq!(entry.activity_id, 1);
    assert_eq!(entry.begin, Some(36_000));
    assert_eq!(entry.duration, Duration::minutes(30));
    assert_eq!(entry.comment.as_deref(), Some("review"));
}

#[test]
fn a_stamp_over_midnight_books_every_day() {
    let app = stamping_out(vec![part(29, 61_200, 86_400), part(30, 0, 32_400)]);

    let records = app.stamp_records().unwrap();

    let dates: Vec<&str> = records.iter().map(|r| r.date.as_str()).collect();
    assert_eq!(dates, ["2026-10-29", "2026-10-30"]);
    let first = records[0].entries.last().unwrap();
    let last = records[1].entries.last().unwrap();
    assert_eq!(
        (first.begin, first.duration),
        (Some(61_200), Duration::hours(7))
    );
    assert_eq!((last.begin, last.duration), (Some(0), Duration::hours(9)));
}

#[test]
fn from_edits_the_first_day_and_to_the_last_day() {
    let mut app = stamping_out(vec![part(29, 61_200, 86_400), part(30, 0, 32_400)]);
    let form = app.edit.as_mut().unwrap();
    form.from = "18:00".into();
    form.to = "08:00".into();

    let records = app.stamp_records().unwrap();

    assert_eq!(records[0].entries.last().unwrap().begin, Some(64_800));
    assert_eq!(
        records[1].entries.last().unwrap().duration,
        Duration::hours(8)
    );
}

#[test]
fn an_overlap_on_any_day_refuses_the_whole_stamp() {
    let mut app = stamping_out(vec![part(29, 61_200, 86_400), part(30, 0, 32_400)]);
    // The 30th already has an entry from 08:00 to 10:00.
    let session = app.stamp_session.as_mut().unwrap();
    let entry = &mut session.records[1].entries[0];
    entry.begin = Some(28_800);
    entry.duration = Duration::hours(2);

    let error = app.stamp_records().unwrap_err();

    assert!(error.contains("Overlaps"), "{error}");
    assert!(error.contains("Fri 30.10."), "{error}");
}

#[test]
fn a_day_before_the_edit_lock_refuses_the_stamp() {
    let mut app = stamping_out(vec![part(30, 36_000, 37_800)]);
    app.edit_lock = Some(day(31));

    let error = app.stamp_records().unwrap_err();

    assert!(error.contains("edit lock"), "{error}");
}

#[test]
fn bad_times_are_refused() {
    let mut app = stamping_out(vec![part(30, 36_000, 37_800)]);
    app.edit.as_mut().unwrap().from = "abc".into();
    assert!(app.stamp_records().unwrap_err().contains("h:mm"));

    let form = app.edit.as_mut().unwrap();
    form.from = "10:30".into();
    form.to = "10:00".into();
    assert!(app.stamp_records().unwrap_err().contains("after From"));
}

#[test]
fn booking_clears_the_stamp_and_its_file() {
    let mut app = stamping_out(vec![part(29, 61_200, 86_400), part(30, 0, 32_400)]);
    app.stamp_store.as_ref().unwrap().save(at(9, 0, 0)).unwrap();

    app.update(Msg::StampBooked(2));

    assert_eq!(app.stamp, None);
    assert_eq!(app.stamp_store.as_ref().unwrap().load().unwrap(), None);
    assert!(
        app.status
            .as_deref()
            .unwrap_or("")
            .contains("Booked 2 entries")
    );
}

#[test]
fn discarding_asks_first() {
    let mut app = stamped_app();
    press(&mut app, "s");

    press(&mut app, "S");
    assert!(app.discard_prompt);
    assert!(render(&app).contains("Discard the running stamp"));

    press(&mut app, "n");
    assert!(!app.discard_prompt);
    assert!(app.stamp.is_some());

    press(&mut app, "Sy");
    assert_eq!(app.stamp, None);
    assert_eq!(app.stamp_store.as_ref().unwrap().load().unwrap(), None);
}

#[test]
fn discarding_without_a_stamp_says_so() {
    let mut app = stamped_app();

    press(&mut app, "S");

    assert!(!app.discard_prompt);
    assert!(app.status.as_deref().unwrap_or("").contains("No stamp"));
}

#[test]
fn stamping_out_needs_activities_and_keeps_the_stamp_otherwise() {
    let mut app = stamped_app();
    app.stamp = Some(at(9, 0, 0));
    app.suitable.clear();
    app.activities.clear();

    app.stamp_out(at(10, 0, 0));

    assert!(
        app.status
            .as_deref()
            .unwrap_or("")
            .contains("No activities")
    );
    assert!(app.stamp.is_some());
}
