use chrono::{NaiveDate, NaiveDateTime};
use rolx_core::activities::Activity;
use rolx_core::stamp::StampState;

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

#[test]
fn stamped_out_offers_the_activity_used_last() {
    let mut state = StampState::default();
    let view = widget_view(&state, at(30, 9, 0), &[meetings()]);
    assert!(!view.stamped_in && !view.timer && !view.stale);
    assert_eq!(view.headline, "Stamped out");
    assert_eq!(view.text, "Tap to stamp in");

    state.stamp_in(at(29, 8, 0), Some(meetings().to_ref()), None);
    state.stamp_out(at(29, 9, 0)).unwrap();
    let view = widget_view(&state, at(30, 9, 0), &[meetings()]);
    assert_eq!(view.text, "Next: #1001.001.01 Cudos - Internal - Meetings");
    // No longer in the list: the hotkey would stamp in without an activity.
    let view = widget_view(&state, at(30, 9, 0), &[]);
    assert_eq!(view.text, "Tap to stamp in");
}

#[test]
fn stamped_in_runs_a_timer_with_the_time_worked() {
    let mut state = StampState::default();
    state.stamp_in(at(30, 8, 12), Some(meetings().to_ref()), None);
    let view = widget_view(&state, at(30, 9, 42), &[meetings()]);
    assert!(view.stamped_in && view.timer && view.running && !view.stale);
    assert_eq!(view.title, "Stamped in since 08:12");
    assert_eq!(view.worked_ms, 90 * 60 * 1000);
    assert_eq!(view.text, "#1001.001.01 Cudos - Internal - Meetings");
}

#[test]
fn break_stops_the_timer() {
    let mut state = StampState::default();
    state.stamp_in(at(30, 8, 0), None, None);
    state.start_break(at(30, 10, 0)).unwrap();
    let view = widget_view(&state, at(30, 10, 30), &[]);
    assert!(view.timer && !view.running);
    assert_eq!(view.title, "On break since 10:00");
    assert_eq!(view.worked_ms, 2 * 3600 * 1000);
    assert_eq!(view.text, "no activity yet");
}

#[test]
fn forgotten_session_opens_the_app() {
    let mut state = StampState::default();
    state.stamp_in(at(29, 8, 0), None, None);
    let view = widget_view(&state, at(30, 9, 0), &[]);
    assert!(view.stamped_in && view.stale && !view.timer);
    assert_eq!(view.title, "Stamped in since Thu 08:00");
}
