use serde_json::{Value, json};

use super::*;
use crate::test_support::{MockServer, Route, start_mock};

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

fn entry(activity: i32, begin: i32, duration: i64) -> DayEntry {
    DayEntry {
        activity_id: activity,
        full_activity_number: format!("A-{activity}"),
        begin: Some(begin),
        duration,
        pause: None,
        comment: None,
    }
}

fn record(entries: &[DayEntry]) -> Value {
    json!({
        "date": "2026-10-30",
        "userId": "u1",
        "dayType": 0,
        "nominalWorkTime": 30000,
        "entries": entries,
    })
}

async fn server(entries: &[DayEntry], lock: &str) -> MockServer {
    start_mock(vec![
        Route::json("GET", "/api/v1/edit-lock", 200, json!({ "date": lock })),
        Route::json("GET", "/api/v1/WorkRecord", 200, json!([record(entries)])),
        Route::empty("PUT", "/api/v1/WorkRecord", 204),
    ])
    .await
}

#[tokio::test]
async fn days_are_listed_chronologically_with_untimed_entries_last() {
    let late = entry(2, 50_400, 3_600);
    let early = entry(1, 28_800, 3_600);
    let untimed = DayEntry {
        begin: None,
        ..entry(3, 0, 7_200)
    };
    // the server lists them in its own order
    let server = server(
        &[late.clone(), untimed.clone(), early.clone()],
        "2026-10-01",
    )
    .await;

    assert_eq!(
        load_day(&server.client(), day()).await.unwrap(),
        vec![early, late, untimed]
    );
}

#[tokio::test]
async fn a_different_order_on_the_server_is_not_a_change() {
    let a = entry(1, 28_800, 3_600);
    let b = entry(2, 36_000, 3_600);
    // the server lists b before a; the user saw (and saves against) the chronological list
    let server = server(&[b.clone(), a.clone()], "2026-10-01").await;

    save_day(
        &server.client(),
        day(),
        &[a.clone(), b.clone()],
        &[b.clone(), a.clone()],
    )
    .await
    .unwrap();

    let sent = &server.requests("PUT")[0].1["entries"];
    assert_eq!(sent[0]["activityId"], 1, "saved in chronological order");
    assert_eq!(sent[1]["activityId"], 2);
}

#[tokio::test]
async fn saves_the_new_entries_and_keeps_the_rest_of_the_record() {
    let a = entry(1, 28_800, 3_600);
    let b = entry(2, 36_000, 3_600);
    let server = server(&[a.clone(), b.clone()], "2026-10-01").await;

    // delete the first entry
    save_day(
        &server.client(),
        day(),
        &[a, b.clone()],
        std::slice::from_ref(&b),
    )
    .await
    .unwrap();

    let puts = server.requests("PUT");
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].0, "/api/v1/WorkRecord/u1/2026-10-30");
    assert_eq!(puts[0].1["dayType"], 0);
    assert_eq!(puts[0].1["nominalWorkTime"], 30000);
    assert_eq!(puts[0].1["entries"].as_array().unwrap().len(), 1);
    assert_eq!(puts[0].1["entries"][0]["activityId"], 2);
}

#[tokio::test]
async fn refuses_to_overwrite_a_day_that_changed_meanwhile() {
    let on_server = entry(1, 28_800, 3_600);
    let seen_by_user = entry(1, 28_800, 1_800);
    let server = server(&[on_server], "2026-10-01").await;

    let err = save_day(&server.client(), day(), &[seen_by_user], &[])
        .await
        .unwrap_err();

    assert!(matches!(err.downcast_ref(), Some(SaveError::Changed)));
    assert!(server.requests("PUT").is_empty());
}

#[tokio::test]
async fn refuses_days_before_the_edit_lock() {
    let a = entry(1, 28_800, 3_600);
    let server = server(std::slice::from_ref(&a), "2026-10-31").await;

    let err = save_day(&server.client(), day(), &[a], &[])
        .await
        .unwrap_err();

    assert!(matches!(err.downcast_ref(), Some(SaveError::Locked(..))));
    assert!(server.requests("PUT").is_empty());
}

#[tokio::test]
async fn loads_the_entries_of_a_day() {
    let a = entry(1, 28_800, 3_600);
    let server = server(std::slice::from_ref(&a), "2026-10-01").await;
    assert_eq!(load_day(&server.client(), day()).await.unwrap(), vec![a]);
}

#[tokio::test]
async fn server_rejection_is_reported_with_its_message() {
    let a = entry(1, 28_800, 3_600);
    let server = start_mock(vec![
        Route::json(
            "GET",
            "/api/v1/edit-lock",
            200,
            json!({ "date": "2026-10-01" }),
        ),
        Route::json(
            "GET",
            "/api/v1/WorkRecord",
            200,
            json!([record(std::slice::from_ref(&a))]),
        ),
        Route::json(
            "PUT",
            "/api/v1/WorkRecord",
            400,
            json!("activity is closed"),
        ),
    ])
    .await;

    let err = save_day(&server.client(), day(), &[a], &[])
        .await
        .unwrap_err();
    assert!(err.to_string().contains("400"));
    assert!(err.to_string().contains("activity is closed"));
}
