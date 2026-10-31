use chrono::{Duration, Local, NaiveDate, TimeZone};
use rolx_core::api::records::Record;
use rolx_core::client::ApiError;

use crate::app::{App, Load, Msg, SyncOutcome};
use crate::outbox::{ChangeState, Outbox, days_equal};
use crate::test_support::*;

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

fn offline_error() -> anyhow::Error {
    ApiError::Network("unreachable".into()).into()
}

/// The shown day without its entries.
fn emptied(app: &App) -> Record {
    let mut record = app.record.clone().unwrap();
    record.entries.clear();
    record
}

/// An app whose only day has one pending change that removed its entry.
fn app_with_pending_delete() -> App {
    let mut app = main_app();
    press(&mut app, "dd");
    app
}

#[test]
fn deleting_without_a_connection_becomes_a_pending_change() {
    let app = app_with_pending_delete();

    let change = app.outbox.get(day()).expect("a pending change");
    assert_eq!(change.state, ChangeState::Pending);
    assert_eq!(change.base.entries.len(), 1);
    assert!(change.record.entries.is_empty());
    assert!(
        app.record.as_ref().unwrap().entries.is_empty(),
        "shown at once"
    );
    assert!(app.status.as_deref().unwrap().contains("offline"));
}

#[test]
fn a_day_missing_from_the_copy_is_refused() {
    let mut app = main_app();
    let record = emptied(&app);

    app.queue_days(vec![record], vec![None], false);

    assert!(app.outbox.is_empty());
    assert!(
        app.status
            .as_deref()
            .unwrap()
            .contains("Not available offline")
    );
}

#[test]
fn a_second_change_keeps_the_first_base() {
    let mut app = app_with_pending_delete();
    let mut again = app.record.clone().unwrap();
    again.paid_leave_type = Some(0);

    app.put(again);

    let change = app.outbox.get(day()).unwrap();
    assert_eq!(app.outbox.len(), 1);
    assert_eq!(
        change.base.entries.len(),
        1,
        "still the day before the first change"
    );
    assert_eq!(change.record.paid_leave_type, Some(0));
}

#[test]
fn undoing_a_change_removes_it() {
    let mut app = main_app();
    let original = app.record.clone().unwrap();
    press(&mut app, "dd");

    app.put(original);

    assert!(app.outbox.is_empty());
}

#[test]
fn the_server_day_is_shown_with_its_pending_change() {
    let mut app = app_with_pending_delete();
    let mut server = emptied(&app);
    server.entries = main_app().record.unwrap().entries;

    app.update(Msg::Day {
        day: day(),
        record: Some(server),
        suitable: Vec::new(),
    });

    assert!(app.record.as_ref().unwrap().entries.is_empty());
    assert_eq!(
        app.copy.day(day()).unwrap().entries.len(),
        1,
        "the copy holds the server's day"
    );
}

#[test]
fn loading_from_the_copy_works_without_a_connection() {
    let mut app = main_app();
    let record = app.record.take().unwrap();
    app.copy.put_days(&[record]);

    app.update(Msg::FromCopy(Load::Day(day())));

    assert_eq!(app.record.as_ref().unwrap().entries.len(), 1);
}

#[test]
fn a_day_missing_from_the_copy_says_so() {
    let mut app = main_app();
    app.record = None;
    app.copy.put_month(
        NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        &[],
        Local::now(),
    );

    app.update(Msg::FromCopy(Load::Day(day())));

    assert!(app.record.is_none());
    assert!(
        app.status
            .as_deref()
            .unwrap()
            .contains("Not available offline")
    );
}

#[test]
fn a_failed_connection_marks_the_app_offline_and_uses_the_copy() {
    let mut app = main_app();
    let record = app.record.take().unwrap();
    app.copy.put_days(&[record]);

    app.update(Msg::LoadFailed(Load::Day(day()), offline_error()));

    assert!(!app.online);
    assert!(app.record.is_some());
}

#[test]
fn a_refused_login_asks_to_log_in_again() {
    let mut app = main_app();

    app.update(Msg::LoadFailed(Load::Header, ApiError::Unauthorized.into()));

    assert!(app.needs_login);
    assert!(app.status.as_deref().unwrap().contains("log in"));
}

#[test]
fn another_failure_is_just_an_error() {
    let mut app = main_app();

    app.update(Msg::LoadFailed(Load::Header, anyhow::anyhow!("boom")));

    assert!(app.online);
    assert!(app.status.as_deref().unwrap().contains("boom"));
}

#[test]
fn a_refused_save_shows_the_servers_message() {
    let mut app = main_app();

    app.update(Msg::SaveFailed("Activity is closed".into()));

    assert_eq!(app.status.as_deref(), Some("Activity is closed"));
}

#[test]
fn an_offline_save_after_a_failed_request_is_queued() {
    let mut app = main_app();
    let record = emptied(&app);
    let base = app.record.clone();

    app.update(Msg::SaveOffline {
        records: vec![record],
        bases: vec![base],
        stamp: false,
        error: offline_error(),
    });

    assert!(!app.online);
    assert_eq!(app.outbox.len(), 1);
}

#[test]
fn a_stamp_booked_offline_ends_the_stamp() {
    let mut app = main_app();
    app.stamp = Some(Local.with_ymd_and_hms(2026, 10, 30, 9, 0, 0).unwrap());
    let mut record = app.record.clone().unwrap();
    record.entries[0].duration = Duration::hours(3);
    let base = app.record.clone();

    app.queue_days(vec![record], vec![base], true);

    assert!(app.stamp.is_none());
    assert_eq!(app.outbox.len(), 1);
    assert!(app.status.as_deref().unwrap().contains("Booked 1 offline"));
}

#[test]
fn an_uploaded_change_leaves_the_outbox_and_enters_the_copy() {
    let mut app = app_with_pending_delete();

    app.finish_sync(vec![(day(), SyncOutcome::Uploaded)]);

    assert!(app.outbox.is_empty());
    assert!(app.copy.day(day()).unwrap().entries.is_empty());
    assert!(
        app.status
            .as_deref()
            .unwrap()
            .contains("Uploaded 1 pending change")
    );
}

#[test]
fn a_day_the_server_already_holds_is_dropped_from_the_outbox() {
    let mut app = app_with_pending_delete();
    let server = emptied(&app);

    app.finish_sync(vec![(day(), SyncOutcome::AlreadySynced(server))]);

    assert!(app.outbox.is_empty());
}

#[test]
fn a_changed_server_day_becomes_a_conflict() {
    let mut app = app_with_pending_delete();
    let server = main_app().record.unwrap();

    app.finish_sync(vec![(day(), SyncOutcome::Conflict(server))]);

    let change = app.outbox.get(day()).unwrap();
    assert_eq!(change.state, ChangeState::Conflict);
    assert!(change.server.is_some());
    assert!(
        app.outbox.upload_order().is_empty(),
        "not retried on its own"
    );
}

#[test]
fn a_refusal_keeps_the_change_with_the_servers_message() {
    let mut app = app_with_pending_delete();

    app.finish_sync(vec![(
        day(),
        SyncOutcome::Rejected("Activity is closed".into()),
    )]);

    let change = app.outbox.get(day()).unwrap();
    assert_eq!(change.state, ChangeState::Rejected);
    assert_eq!(change.message.as_deref(), Some("Activity is closed"));
}

#[test]
fn losing_the_connection_while_syncing_keeps_everything() {
    let mut app = app_with_pending_delete();
    app.syncing = true;

    app.finish_sync(vec![(day(), SyncOutcome::Offline)]);

    assert!(!app.online);
    assert!(!app.syncing);
    assert_eq!(app.outbox.get(day()).unwrap().state, ChangeState::Pending);
}

#[test]
fn an_expired_login_stops_the_upload_and_asks_to_log_in() {
    let mut app = app_with_pending_delete();

    app.finish_sync(vec![(day(), SyncOutcome::Unauthorized)]);

    assert!(app.needs_login);
    assert_eq!(app.outbox.len(), 1);
}

#[test]
fn the_pending_changes_survive_a_restart() {
    let store = temp_local_store();
    let mut app = main_app();
    app.install_store(store.clone());
    press(&mut app, "dd");

    let mut restarted = main_app();
    restarted.install_store(store);

    assert_eq!(restarted.outbox.len(), 1);
    let change = restarted.outbox.get(day()).unwrap();
    assert!(change.record.entries.is_empty());
}

#[test]
fn an_unreadable_outbox_is_kept_aside_not_overwritten() {
    let store = temp_local_store();
    store.write("outbox.json", &"not a list").unwrap();
    let mut app = main_app();

    app.install_store(store.clone());

    assert!(app.outbox.is_empty());
    assert!(store.root().join("outbox.damaged.json").exists());
    assert!(app.status.as_deref().unwrap().contains("unreadable"));
}

#[test]
fn the_header_shows_the_connection_and_the_pending_changes() {
    let mut app = app_with_pending_delete();
    app.status = None;
    assert!(render(&app).contains("1 pending"));
    assert!(!render(&app).contains("offline"));

    app.online = false;
    assert!(render(&app).contains("offline"));

    app.needs_login = true;
    assert!(render(&app).contains("log in (L)"));
}

#[test]
fn a_change_saved_in_the_copy_loads_again_after_a_restart() {
    let store = temp_local_store();
    let mut app = main_app();
    app.install_store(store.clone());
    let record = app.record.clone().unwrap();
    app.update(Msg::Learned(vec![record]));

    let mut restarted = main_app();
    restarted.install_store(store);

    assert!(restarted.copy.day(day()).is_some());
    let persisted: Outbox = Outbox::load(restarted.store.as_ref().unwrap()).unwrap();
    assert!(persisted.is_empty());
    assert!(days_equal(
        restarted.copy.day(day()).unwrap(),
        &main_app().record.unwrap()
    ));
}

#[test]
fn r_with_nothing_to_upload_says_so_and_l_when_logged_in_does_nothing() {
    let mut app = main_app();

    press(&mut app, "R");
    assert!(app.status.is_none(), "no client, nothing to sync");

    press(&mut app, "L");
    assert!(app.status.as_deref().unwrap().contains("Already logged in"));
}
