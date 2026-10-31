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
    assert_eq!(view.text, "Choose an activity");

    state.stamp_in(at(29, 8, 0), Some(meetings().to_ref()), None);
    state.stamp_out(at(29, 9, 0)).unwrap();
    let view = widget_view(&state, at(30, 9, 0), &[meetings()]);
    assert_eq!(view.text, "Next: #1001.001.01 Cudos - Internal - Meetings");
    // No longer in the list: the hotkey would stamp in without an activity.
    let view = widget_view(&state, at(30, 9, 0), &[]);
    assert_eq!(view.text, "Choose an activity");
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

fn training() -> Activity {
    Activity {
        id: 8,
        full_number: "#1001.001.02".into(),
        full_name: "Cudos - Internal - Training".into(),
        ..Default::default()
    }
}

#[test]
fn picker_lists_recent_first_and_marks_the_running_one() {
    let mut state = StampState::default();
    state.stamp_in(at(30, 8, 0), Some(training().to_ref()), None);
    let items = picker_items(&[meetings(), training()], &state, "");
    assert_eq!(items[0].id, 8);
    assert_eq!(items[0].group, "recent");
    assert!(items[0].current);
    assert_eq!(items[0].short, "Training");
    assert_eq!(
        (items[1].id, items[1].group, items[1].current),
        (7, "other", false)
    );

    let items = picker_items(&[meetings(), training()], &state, "meet");
    assert_eq!(items.iter().map(|i| i.id).collect::<Vec<_>>(), [7]);
}

#[test]
fn picking_stamps_in_switches_or_keeps_the_session() {
    let mut state = StampState::default();
    assert_eq!(
        start_on(&mut state, at(30, 8, 0), meetings().to_ref()),
        Ok(Started::StampedIn)
    );
    assert_eq!(
        start_on(&mut state, at(30, 9, 0), meetings().to_ref()),
        Ok(Started::Unchanged)
    );
    assert!(state.pending.is_empty());

    assert_eq!(
        start_on(&mut state, at(30, 10, 0), training().to_ref()),
        Ok(Started::Switched)
    );
    assert_eq!(state.pending.len(), 1);
    assert_eq!(
        state.open.as_ref().unwrap().activity,
        Some(training().to_ref())
    );
}

#[test]
fn picking_gives_a_session_without_activity_one() {
    let mut state = StampState::default();
    state.stamp_in(at(30, 8, 0), None, None);
    assert_eq!(
        start_on(&mut state, at(30, 9, 0), meetings().to_ref()),
        Ok(Started::Unchanged)
    );
    assert!(state.pending.is_empty());
    let open = state.open.as_ref().unwrap();
    assert_eq!(
        (open.start, open.activity.clone()),
        (at(30, 8, 0), Some(meetings().to_ref()))
    );
}
