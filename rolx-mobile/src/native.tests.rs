use chrono::{NaiveDate, NaiveDateTime};
use rolx_core::activities::Activity;
use rolx_core::stamp::{ActivityRef, StampState};

use super::*;

fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, day)
        .unwrap()
        .and_hms_opt(hour, minute, 0)
        .unwrap()
}

fn meetings() -> Activity {
    Activity {
        id: 7,
        full_number: "#1001.001.01".into(),
        full_name: "Cudos - Internal - Meetings".into(),
        ..Default::default()
    }
}

fn stamped_in(activity: Option<ActivityRef>) -> StampState {
    let mut state = StampState::default();
    state.stamp_in(at(30, 8, 12), activity, None);
    state
}

const NOW_MS: i64 = 1_000_000_000;

#[test]
fn no_notification_when_stamped_out() {
    let state = StampState::default();
    assert_eq!(stamp_notification(&state, at(30, 9, 0), NOW_MS, &[]), None);
}

#[test]
fn stamped_in_shows_activity_and_a_timer_from_the_start() {
    let state = stamped_in(Some(meetings().to_ref()));
    let shown = stamp_notification(&state, at(30, 9, 42), NOW_MS, &[meetings()]).unwrap();
    assert_eq!(shown.title, "Stamped in since 08:12");
    assert_eq!(shown.text, "#1001.001.01 Cudos - Internal - Meetings");
    // 1:30 worked so far.
    assert_eq!(shown.chronometer_base_ms, Some(NOW_MS - 90 * 60 * 1000));
}

#[test]
fn unknown_activity_shows_its_number_and_none_says_so() {
    let state = stamped_in(Some(meetings().to_ref()));
    let shown = stamp_notification(&state, at(30, 9, 0), NOW_MS, &[]).unwrap();
    assert_eq!(shown.text, "#1001.001.01");

    let state = stamped_in(None);
    let shown = stamp_notification(&state, at(30, 9, 0), NOW_MS, &[]).unwrap();
    assert_eq!(shown.text, "no activity yet");
}

#[test]
fn break_shows_its_start_and_no_timer() {
    let mut state = stamped_in(None);
    state.start_break(at(30, 10, 15)).unwrap();
    let shown = stamp_notification(&state, at(30, 10, 40), NOW_MS, &[]).unwrap();
    assert_eq!(shown.title, "Stamped in since 08:12");
    assert_eq!(shown.text, "On break since 10:15");
    assert_eq!(shown.chronometer_base_ms, None);
}

#[test]
fn timer_after_a_break_leaves_the_break_out() {
    let mut state = stamped_in(None);
    state.start_break(at(30, 10, 0)).unwrap();
    state.end_break(at(30, 10, 30)).unwrap();
    let shown = stamp_notification(&state, at(30, 11, 12), NOW_MS, &[]).unwrap();
    // 08:12 to 11:12 minus the 30 minute break.
    assert_eq!(shown.chronometer_base_ms, Some(NOW_MS - 150 * 60 * 1000));
}

#[test]
fn session_from_another_day_names_the_weekday() {
    let state = stamped_in(None);
    let shown = stamp_notification(&state, at(31, 7, 0), NOW_MS, &[]).unwrap();
    assert_eq!(shown.title, "Stamped in since Fri 08:12");
}
