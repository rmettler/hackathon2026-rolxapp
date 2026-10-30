use super::*;
use crate::test_support::{MockServer, Route, start_mock};

async fn day_server(record: Value, put_status: u16) -> MockServer {
    start_mock(vec![
        Route::json("GET", "/api/v1/WorkRecord", 200, json!([record])),
        Route::empty("PUT", "/api/v1/WorkRecord", put_status),
    ])
    .await
}

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

fn activity(id: i32) -> ActivityRef {
    ActivityRef {
        id,
        full_number: format!("A-{id}"),
    }
}

fn pending(begin: i32, activity: Option<ActivityRef>) -> PendingEntry {
    PendingEntry {
        date: day(),
        begin,
        duration: 3600,
        pause: 0,
        activity,
        comment: Some("work".into()),
    }
}

fn server_record(entries: Value) -> Value {
    json!({
        "date": "2026-10-30",
        "userId": "u1",
        "dayType": 0,
        "dayName": "Friday",
        "nominalWorkTime": 30000,
        "entries": entries,
    })
}

fn temp_state(name: &str, state: &StampState) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("rolx-sync-test-{}-{name}.json", std::process::id()));
    state.save_to(&path).unwrap();
    path
}

#[tokio::test]
async fn appends_to_the_day_record_and_keeps_unassigned_entries() {
    let existing = json!({"activityId": 5, "fullActivityNumber": "A-5", "duration": 1800, "begin": 28800, "pause": null, "comment": null});
    let server = day_server(server_record(json!([existing])), 204).await;
    let state = StampState {
        open: None,
        pending: vec![pending(36000, Some(activity(1))), pending(40000, None)],
        ..StampState::default()
    };
    let path = temp_state("append", &state);

    let report = run(&server.client(), &path).await.unwrap();

    assert_eq!(report.uploaded, 1);
    assert_eq!(report.unassigned, 1);
    assert!(report.failed.is_empty());

    let puts = server.requests("PUT");
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].0, "/api/v1/WorkRecord/u1/2026-10-30");
    let sent = &puts[0].1;
    assert_eq!(
        sent["dayType"], 0,
        "other fields of the record are sent back unchanged"
    );
    assert_eq!(sent["nominalWorkTime"], 30000);
    let entries = sent["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["activityId"], 5);
    assert_eq!(entries[1]["activityId"], 1);
    assert_eq!(entries[1]["begin"], 36000);
    assert_eq!(entries[1]["duration"], 3600);
    assert_eq!(entries[1]["comment"], "work");

    let left = StampState::load_from(&path).unwrap();
    assert_eq!(left.pending, vec![pending(40000, None)]);
    std::fs::remove_file(&path).unwrap();
}

#[tokio::test]
async fn does_not_upload_entries_that_are_already_on_the_server() {
    let already_there = json!({"activityId": 1, "fullActivityNumber": "A-1", "duration": 3600, "begin": 36000, "pause": null, "comment": "work"});
    let server = day_server(server_record(json!([already_there])), 204).await;
    let state = StampState {
        open: None,
        pending: vec![pending(36000, Some(activity(1)))],
        ..StampState::default()
    };
    let path = temp_state("idempotent", &state);

    let report = run(&server.client(), &path).await.unwrap();

    assert_eq!(report.uploaded, 1);
    assert!(server.requests("PUT").is_empty(), "nothing new to send");
    assert!(StampState::load_from(&path).unwrap().pending.is_empty());
    std::fs::remove_file(&path).unwrap();
}

#[tokio::test]
async fn failed_upload_keeps_entries_queued() {
    let server = day_server(server_record(json!([])), 400).await;
    let state = StampState {
        open: None,
        pending: vec![pending(36000, Some(activity(1)))],
        ..StampState::default()
    };
    let path = temp_state("failure", &state);

    let report = run(&server.client(), &path).await.unwrap();

    assert_eq!(report.uploaded, 0);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].0, day());
    assert!(report.failed[0].1.contains("400"));
    assert_eq!(StampState::load_from(&path).unwrap().pending.len(), 1);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn pause_is_only_sent_when_there_was_a_break() {
    let mut with_pause = pending(0, Some(activity(1)));
    with_pause.pause = 600;
    assert_eq!(entry_json(&with_pause, &activity(1))["pause"], 600);
    assert!(entry_json(&pending(0, None), &activity(1))["pause"].is_null());
}
