use serde_json::json;

use super::*;
use crate::stamp::ActivityRef;
use crate::test_support::{Route, start_mock};

fn d(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
}

fn nov(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, day).unwrap()
}

fn entry(activity: i32, hours: i64) -> serde_json::Value {
    json!({"activityId": activity, "fullActivityNumber": format!("A-{activity}"), "duration": hours * 3600, "begin": 28800, "pause": null, "comment": null})
}

/// A server record. 7.56 h is 90 % of 8.4 h.
fn record(
    date: NaiveDate,
    nominal: i64,
    leave: Option<&str>,
    entries: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "date": date.to_string(),
        "userId": "u1",
        "dayType": 0,
        "nominalWorkTime": nominal,
        "paidLeaveType": leave,
        "entries": entries,
    })
}

const NINETY: i64 = 27_216;

/// Mon 26.10. - Sun 01.11.: 4 working days at 90 %, Wednesday is a public holiday.
fn sample_week() -> Week {
    parse_week(&[
        record(d(26), NINETY, None, vec![entry(1, 8), entry(2, 1)]),
        record(d(27), NINETY, None, vec![entry(1, 7)]),
        record(d(28), 0, None, vec![]), // holiday
        record(d(29), NINETY, None, vec![entry(2, 4)]),
        record(d(30), NINETY, None, vec![]),
        record(d(31), 0, None, vec![]),
        record(nov(1), 0, None, vec![]),
    ])
    .unwrap()
}

#[test]
fn week_starts_on_monday() {
    assert_eq!(week_start(d(26)), d(26));
    assert_eq!(week_start(d(30)), d(26));
    assert_eq!(
        week_start(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()),
        d(26)
    );
    assert_eq!(
        week_start(NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()),
        NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
    );
}

#[test]
fn daily_target_is_eight_point_four_hours() {
    assert_eq!(DAILY_TARGET_SECS, 8 * 3600 + 24 * 60);
}

#[test]
fn target_is_the_sum_of_the_servers_daily_nominal_times() {
    let s = summarize(&sample_week(), &[]);
    assert_eq!(s.target_secs, 4 * NINETY, "holiday and weekend count as 0");
    assert_eq!(s.pensum_percent, Some(90));
}

#[test]
fn booked_and_remaining_time() {
    let s = summarize(&sample_week(), &[]);
    assert_eq!(s.booked_secs, (8 + 1 + 7 + 4) * 3600);
    assert_eq!(s.remaining_secs, 4 * NINETY - 20 * 3600);
    assert_eq!(s.days[0].booked_secs, 9 * 3600);
}

#[test]
fn exceeding_the_target_gives_negative_remaining_time() {
    let week = parse_week(&[record(d(26), NINETY, None, vec![entry(1, 10)])]).unwrap();
    assert!(summarize(&week, &[]).remaining_secs < 0);
}

#[test]
fn paid_leave_counts_as_worked() {
    let week = parse_week(&[
        record(d(26), NINETY, Some("Vacation"), vec![]),
        record(d(27), NINETY, None, vec![entry(1, 7)]),
    ])
    .unwrap();
    let s = summarize(&week, &[]);
    assert_eq!(s.leave_secs, NINETY);
    assert_eq!(s.remaining_secs, 2 * NINETY - NINETY - 7 * 3600);
}

#[test]
fn queued_entries_count_for_their_day() {
    let queued = vec![
        PendingEntry {
            date: d(30),
            begin: 28800,
            duration: 2 * 3600,
            pause: 0,
            activity: Some(ActivityRef {
                id: 3,
                full_number: "A-3".into(),
            }),
            comment: None,
        },
        PendingEntry {
            date: d(30),
            begin: 40000,
            duration: 1800,
            pause: 0,
            activity: None,
            comment: None,
        },
        // a day outside this week is ignored
        PendingEntry {
            date: NaiveDate::from_ymd_opt(2026, 11, 5).unwrap(),
            begin: 0,
            duration: 3600,
            pause: 0,
            activity: None,
            comment: None,
        },
    ];
    let s = summarize(&sample_week(), &queued);
    let friday = &s.days[4];
    assert_eq!(friday.booked_secs, 2 * 3600 + 1800);
    assert!(
        friday.by_activity.iter().any(|t| t.activity_id == 0),
        "unassigned shows as id 0"
    );
    assert_eq!(s.booked_secs, (8 + 1 + 7 + 4) * 3600 + 2 * 3600 + 1800);
}

#[test]
fn activities_are_totalled_largest_first() {
    let s = summarize(&sample_week(), &[]);
    let totals: Vec<_> = s
        .activities
        .iter()
        .map(|t| (t.activity_id, t.secs / 3600))
        .collect();
    assert_eq!(totals, vec![(1, 15), (2, 5)]);
    assert_eq!(s.days[0].by_activity[0].activity_id, 1);
}

#[test]
fn a_week_without_working_days_has_no_pensum() {
    let week = parse_week(&[
        record(d(31), 0, None, vec![]),
        record(nov(1), 0, None, vec![]),
    ])
    .unwrap();
    let s = summarize(&week, &[]);
    assert_eq!(s.pensum_percent, None);
    assert_eq!(s.target_secs, 0);
}

#[tokio::test]
async fn fetches_the_whole_week_around_a_day() {
    let records: Vec<_> = (26..=31)
        .map(|day| record(d(day), NINETY, None, vec![]))
        .chain([record(nov(1), 0, None, vec![])])
        .collect();
    let server = start_mock(vec![Route::json(
        "GET",
        "/api/v1/WorkRecord/u1/range/2026-10-26..2026-11-02",
        200,
        json!(records),
    )])
    .await;

    let week = fetch_week(&server.client(), d(30)).await.unwrap();
    assert_eq!(week.days.len(), 7);
    assert_eq!(week.days[0].date, d(26));
}
