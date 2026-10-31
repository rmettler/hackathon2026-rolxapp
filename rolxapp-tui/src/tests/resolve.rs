use chrono::{Duration, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use rolx_core::api::records::{Record, RecordEntry};

use crate::app::{App, DayView, SyncOutcome};
use crate::outbox::ChangeState;
use crate::test_support::*;

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

fn extra_entry() -> RecordEntry {
    RecordEntry {
        activity_id: 2,
        full_activity_number: "P1-A2".into(),
        duration: Duration::minutes(30),
        begin: None,
        pause: None,
        comment: None,
    }
}

/// The day on the server: the original entry plus one more.
fn server_day() -> Record {
    let mut record = main_app().record.unwrap();
    record.entries.push(extra_entry());
    record
}

/// A day with a pending change that removed its only entry; the copy holds the original.
fn app_with_pending_delete() -> App {
    let mut app = main_app();
    app.copy.put_days(&[app.record.clone().unwrap()]);
    press(&mut app, "dd");
    app
}

/// The pending delete after the server turned out to have an extra entry.
fn app_with_conflict() -> App {
    let mut app = app_with_pending_delete();
    app.finish_sync(vec![(day(), SyncOutcome::Conflict(server_day()))]);
    app
}

fn enter(app: &mut App) {
    special(app, KeyCode::Enter, KeyModifiers::NONE);
}

fn escape(app: &mut App) {
    special(app, KeyCode::Esc, KeyModifiers::NONE);
}

#[test]
fn o_lists_the_pending_changes() {
    let mut app = app_with_pending_delete();

    press(&mut app, "O");

    assert!(app.outbox_popup.is_some());
    let screen = render(&app);
    assert!(screen.contains("Pending changes"));
    assert!(screen.contains("30.10.2026"));
    escape(&mut app);
    assert!(app.outbox_popup.is_none());
}

#[test]
fn o_with_nothing_pending_says_so() {
    let mut app = main_app();

    press(&mut app, "O");

    assert!(app.outbox_popup.is_none());
    assert_eq!(app.status.as_deref(), Some("No pending changes"));
}

#[test]
fn enter_on_a_pending_day_opens_that_day() {
    let mut app = app_with_pending_delete();
    app.day = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
    app.record = None;

    press(&mut app, "O");
    enter(&mut app);

    assert_eq!(app.day, day());
    assert!(app.outbox_popup.is_none());
    assert!(
        app.record.as_ref().unwrap().entries.is_empty(),
        "with the change"
    );
}

#[test]
fn a_rejected_change_shows_the_servers_message_and_can_be_retried() {
    let mut app = app_with_pending_delete();
    app.finish_sync(vec![(
        day(),
        SyncOutcome::Rejected("Activity is closed".into()),
    )]);

    press(&mut app, "O");
    assert!(render(&app).contains("Activity is closed"));
    press(&mut app, "r");

    let change = app.outbox.get(day()).unwrap();
    assert_eq!(change.state, ChangeState::Pending);
    assert_eq!(change.message, None);
}

#[test]
fn saving_a_rejected_day_again_makes_it_pending() {
    let mut app = app_with_pending_delete();
    app.finish_sync(vec![(
        day(),
        SyncOutcome::Rejected("Activity is closed".into()),
    )]);
    let mut record = app.record.clone().unwrap();
    record.entries.push(extra_entry());

    app.put(record);

    assert_eq!(app.outbox.get(day()).unwrap().state, ChangeState::Pending);
}

#[test]
fn discarding_asks_first_and_restores_the_copys_day() {
    let mut app = app_with_pending_delete();

    press(&mut app, "Od");
    press(&mut app, "n");
    assert_eq!(app.outbox.len(), 1, "any other key keeps it");

    press(&mut app, "dy");
    assert!(app.outbox.is_empty());
    assert!(app.outbox_popup.is_none());
    assert_eq!(app.record.as_ref().unwrap().entries.len(), 1);
}

#[test]
fn enter_on_a_conflict_opens_the_dialog_with_merge_preselected() {
    let mut app = app_with_conflict();

    press(&mut app, "O");
    enter(&mut app);

    let dialog = app.conflict.as_ref().unwrap();
    assert_eq!(dialog.choice, 0);
    assert!(dialog.merged.is_ok());
    let screen = render(&app);
    assert!(screen.contains("Yours"));
    assert!(screen.contains("On the server"));
    assert!(screen.contains("Merge"));
}

#[test]
fn merging_keeps_my_change_on_top_of_the_servers_day() {
    let mut app = app_with_conflict();
    press(&mut app, "O");
    enter(&mut app);

    enter(&mut app);

    assert!(app.conflict.is_none());
    let change = app.outbox.get(day()).unwrap();
    assert_eq!(change.state, ChangeState::Pending);
    assert_eq!(
        change.base.entries.len(),
        2,
        "builds on the server's day now"
    );
    assert_eq!(change.record.entries.len(), 1);
    assert_eq!(change.record.entries[0].full_activity_number, "P1-A2");
    assert_eq!(app.record.as_ref().unwrap().entries.len(), 1);
}

#[test]
fn keeping_mine_uploads_my_day_over_the_servers() {
    let mut app = app_with_conflict();
    press(&mut app, "O");
    enter(&mut app);

    press(&mut app, "2");
    enter(&mut app);

    let change = app.outbox.get(day()).unwrap();
    assert_eq!(change.state, ChangeState::Pending);
    assert_eq!(change.base.entries.len(), 2);
    assert!(change.record.entries.is_empty());
}

#[test]
fn keeping_theirs_drops_my_change() {
    let mut app = app_with_conflict();
    press(&mut app, "O");
    enter(&mut app);

    press(&mut app, "3");
    enter(&mut app);

    assert!(app.outbox.is_empty());
    assert_eq!(app.record.as_ref().unwrap().entries.len(), 2);
    assert_eq!(app.copy.day(day()).unwrap().entries.len(), 2);
}

#[test]
fn a_merge_that_cannot_work_is_not_offered_first() {
    let mut app = app_with_pending_delete();
    let mut theirs = main_app().record.unwrap();
    theirs.entries[0].duration = Duration::minutes(120);
    app.finish_sync(vec![(day(), SyncOutcome::Conflict(theirs))]);
    press(&mut app, "O");
    enter(&mut app);

    assert_eq!(app.conflict.as_ref().unwrap().choice, 1);
    assert!(render(&app).contains("Merge not possible"));
    press(&mut app, "1");
    enter(&mut app);

    assert!(app.conflict.is_some(), "stays open");
    assert!(app.status.as_deref().unwrap().contains("Can't merge"));
}

#[test]
fn escape_leaves_the_conflict_for_later() {
    let mut app = app_with_conflict();
    press(&mut app, "O");
    enter(&mut app);

    escape(&mut app);

    assert!(app.conflict.is_none());
    assert_eq!(app.outbox.get(day()).unwrap().state, ChangeState::Conflict);
}

#[test]
fn the_header_says_how_many_changes_need_attention() {
    let mut app = app_with_conflict();
    app.status = None;

    assert!(render(&app).contains("1 need attention (O)"));
}

#[test]
fn the_day_view_marks_a_pending_day_and_its_new_entries() {
    let mut app = main_app();
    let mut record = app.record.clone().unwrap();
    record.entries.push(extra_entry());
    app.copy.put_days(&[app.record.clone().unwrap()]);

    app.put(record);

    let screen = render(&app);
    assert!(screen.contains("[↑ pending]"));
    assert!(screen.contains("↑ P1-A2"));
    assert!(!screen.contains("↑ P1-A1"));
}

#[test]
fn the_week_view_marks_pending_days_in_a_sync_row() {
    let mut app = app_with_pending_delete();
    app.day_view = DayView::Week;
    app.week = vec![app.record.clone().unwrap()];

    let screen = render(&app);

    assert!(screen.contains("Sync"));
    assert!(screen.contains("↑"));
}
