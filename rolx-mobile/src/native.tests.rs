use chrono::{NaiveDate, NaiveDateTime};
use rolx_core::activities::Activity;
use rolx_core::dayplan::DayEntry;
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

#[test]
fn vibrations_are_one_and_two_pulses() {
    assert_eq!(Vibration::StampIn.timings_ms().len(), 2);
    assert_eq!(Vibration::StampOut.timings_ms().len(), 4);
    for kind in [Vibration::StampIn, Vibration::StampOut] {
        assert_eq!(kind.timings_ms()[0], 0, "starts right away");
    }
}

fn entry(activity_id: i32, number: &str, begin: Option<i32>, duration: i64) -> DayEntry {
    DayEntry {
        activity_id,
        full_activity_number: number.into(),
        begin,
        duration,
        pause: None,
        comment: None,
    }
}

#[test]
fn day_share_lists_entries_in_order_with_the_total() {
    let mut review = entry(9, "#2042.001.02", Some(13 * 3600), 5400);
    review.pause = Some(600);
    review.comment = Some("Code review".into());
    let entries = [
        entry(9, "#2042.001.02", None, 1800),
        review,
        entry(7, "#1001.001.01", Some(8 * 3600 + 720), 3600),
    ];
    let shared = day_share(
        NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
        &entries,
        &[meetings()],
    );
    assert_eq!(shared.subject, "RolX Fri 30.10.2026");
    assert_eq!(
        shared.text,
        "RolX Fri 30.10.2026\n\
         08:12-09:12 · 1:00 · #1001.001.01 Cudos - Internal - Meetings\n\
         13:00-14:40 · 1:30 · #2042.001.02 · Code review\n\
         0:30 · #2042.001.02\n\
         Total 3:00"
    );
}

#[test]
fn day_share_of_an_empty_day() {
    let shared = day_share(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(), &[], &[]);
    assert_eq!(shared.text, "RolX Sat 31.10.2026\nNo entries.\nTotal 0:00");
}

fn week(days: Vec<(i64, bool, Vec<DayEntry>)>) -> rolx_core::week::Week {
    let monday = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
    rolx_core::week::Week {
        days: days
            .into_iter()
            .enumerate()
            .map(
                |(i, (nominal_secs, on_leave, entries))| rolx_core::week::WeekDay {
                    date: monday + chrono::Duration::days(i as i64),
                    nominal_secs,
                    on_leave,
                    entries,
                },
            )
            .collect(),
    }
}

const DAY: i64 = 30_240;

#[test]
fn week_share_shows_target_days_and_activities() {
    let week = week(vec![
        (
            DAY,
            false,
            vec![entry(7, "#1001.001.01", Some(8 * 3600), DAY)],
        ),
        (DAY, true, vec![]),
        (DAY, false, vec![entry(9, "#2042.001.02", None, 3600)]),
        (DAY, false, vec![]),
        (DAY, false, vec![]),
        (0, false, vec![entry(7, "#1001.001.01", None, 1800)]),
        (0, false, vec![]),
    ]);
    let queued = [rolx_core::stamp::PendingEntry {
        date: NaiveDate::from_ymd_opt(2026, 10, 29).unwrap(),
        begin: 9 * 3600,
        duration: 900,
        pause: 0,
        activity: None,
        comment: None,
    }];
    let summary = rolx_core::week::summarize(&week, &queued);
    let shared = week_share(&summary, &[meetings()]);
    assert_eq!(shared.subject, "RolX week 26.10.-01.11.2026");
    assert_eq!(
        shared.text,
        "RolX week 26.10.-01.11.2026\n\
         Booked 10:09 of 42:00 · 23:27 left\n\
         Leave 8:24\n\
         \n\
         Mon 26.10. 8:24 of 8:24\n\
         Tue 27.10. 0:00 of 8:24 · leave\n\
         Wed 28.10. 1:00 of 8:24\n\
         Thu 29.10. 0:15 of 8:24\n\
         Fri 30.10. 0:00 of 8:24\n\
         Sat 31.10. 0:30 of 0:00\n\
         \n\
         8:54 · #1001.001.01 Cudos - Internal - Meetings\n\
         1:00 · #2042.001.02\n\
         0:15 · no activity yet"
    );
}

#[test]
fn week_share_says_when_the_target_is_exceeded() {
    let week = week(vec![(
        DAY,
        false,
        vec![entry(9, "#2042.001.02", None, DAY + 3600)],
    )]);
    let summary = rolx_core::week::summarize(&week, &[]);
    let shared = week_share(&summary, &[]);
    assert!(
        shared.text.contains("Booked 9:24 of 8:24 · 1:00 over"),
        "{}",
        shared.text
    );
}
