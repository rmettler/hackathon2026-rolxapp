use serde_json::json;

use super::*;
use crate::test_support::{Route, start_mock};

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

fn sample() -> Balance {
    parse_balance(&json!({
        "byDate": "2026-10-30",
        "overtime": 12_000,
        "vacationAvailable": 453_600,
        "vacationAvailableDays": 12.5,
        "vacationPlanned": 54_432,
        "vacationPlannedDays": 2,
    }))
    .unwrap()
}

#[test]
fn parses_the_servers_balance() {
    let b = sample();
    assert_eq!(b.by_date, day());
    assert_eq!(b.overtime_secs, 12_000);
    assert_eq!(b.vacation_available_days, Some(12.5));
    assert_eq!(b.vacation_planned_secs, 54_432);
}

#[test]
fn overtime_is_signed() {
    let mut b = sample();
    assert_eq!(b.overtime_text(), "+3:20 h");
    b.overtime_secs = -5_400;
    assert_eq!(b.overtime_text(), "-1:30 h");
    b.overtime_secs = 0;
    assert_eq!(b.overtime_text(), "0:00 h");
}

#[test]
fn small_negative_overtime_keeps_its_sign() {
    let mut b = sample();
    b.overtime_secs = -600;
    assert_eq!(b.overtime_text(), "-0:10 h");
}

#[test]
fn vacation_shows_days_and_planned_days() {
    assert_eq!(sample().vacation_text(), "12.5 days (2 planned)");
}

#[test]
fn vacation_without_planned_time_omits_it() {
    let mut b = sample();
    b.vacation_planned_secs = 0;
    b.vacation_available_days = Some(1.0);
    assert_eq!(b.vacation_text(), "1 day");
}

#[test]
fn vacation_falls_back_to_hours_without_a_day_length() {
    let b = parse_balance(&json!({
        "byDate": "2026-10-30",
        "overtime": 0,
        "vacationAvailable": 90_000,
        "vacationAvailableDays": null,
        "vacationPlanned": 3_600,
        "vacationPlannedDays": null,
    }))
    .unwrap();
    assert_eq!(b.vacation_text(), "25:00 h (1:00 h planned)");
}

#[test]
fn days_drop_trailing_zeros() {
    assert_eq!(fmt_days(12.0), "12");
    assert_eq!(fmt_days(0.25), "0.25");
    assert_eq!(fmt_days(7.456), "7.46");
}

#[tokio::test]
async fn fetches_the_balance_of_a_day() {
    let server = start_mock(vec![Route::json(
        "GET",
        "/api/v1/Balance/2026-10-30",
        200,
        json!({
            "byDate": "2026-10-30",
            "overtime": 3_600,
            "vacationAvailable": 0,
            "vacationAvailableDays": 0.0,
            "vacationPlanned": 0,
            "vacationPlannedDays": 0.0,
        }),
    )])
    .await;

    let b = fetch_balance(&server.client(), day()).await.unwrap();
    assert_eq!(b.overtime_secs, 3_600);
}
