use super::*;

fn at(day: u32, h: u32, m: u32, s: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, day)
        .unwrap()
        .and_hms_opt(h, m, s)
        .unwrap()
}

fn act(id: i32) -> ActivityRef {
    ActivityRef {
        id,
        full_number: format!("A-{id}"),
    }
}

#[test]
fn stamp_in_and_out_creates_one_entry() {
    let mut s = StampState::default();
    assert_eq!(s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None), 0);
    assert_eq!(s.stamp_out(at(30, 12, 30, 0)), Ok(1));

    assert!(s.open.is_none());
    assert_eq!(
        s.pending,
        vec![PendingEntry {
            date: at(30, 0, 0, 0).date(),
            begin: 9 * 3600,
            duration: 3 * 3600 + 1800,
            pause: 0,
            activity: Some(act(1)),
            comment: None,
        }]
    );
}

#[test]
fn stamp_out_without_stamp_in_fails() {
    let mut s = StampState::default();
    assert_eq!(s.stamp_out(at(30, 9, 0, 0)), Err(StampError::NotStampedIn));
}

#[test]
fn stamp_in_while_stamped_in_closes_previous_session() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    assert_eq!(s.stamp_in(at(30, 11, 0, 0), Some(act(2)), None), 1);

    assert_eq!(s.pending.len(), 1);
    assert_eq!(s.pending[0].duration, 2 * 3600);
    assert_eq!(s.open.as_ref().unwrap().start, at(30, 11, 0, 0));
}

#[test]
fn break_is_subtracted_and_reported_as_pause() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 8, 0, 0), Some(act(1)), None);
    s.start_break(at(30, 12, 0, 0)).unwrap();
    s.end_break(at(30, 12, 45, 0)).unwrap();
    s.stamp_out(at(30, 17, 0, 0)).unwrap();

    let e = &s.pending[0];
    assert_eq!(e.begin, 8 * 3600);
    assert_eq!(e.pause, 45 * 60);
    assert_eq!(e.duration, 9 * 3600 - 45 * 60);
}

#[test]
fn stamp_out_during_break_ends_the_break_first() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 8, 0, 0), Some(act(1)), None);
    s.start_break(at(30, 16, 0, 0)).unwrap();
    s.stamp_out(at(30, 16, 30, 0)).unwrap();

    let e = &s.pending[0];
    assert_eq!(e.pause, 30 * 60);
    assert_eq!(e.duration, 8 * 3600);
}

#[test]
fn break_state_errors() {
    let mut s = StampState::default();
    assert_eq!(
        s.start_break(at(30, 9, 0, 0)),
        Err(StampError::NotStampedIn)
    );
    s.stamp_in(at(30, 9, 0, 0), None, None);
    assert_eq!(s.end_break(at(30, 9, 5, 0)), Err(StampError::NotOnBreak));
    s.start_break(at(30, 9, 10, 0)).unwrap();
    assert_eq!(
        s.start_break(at(30, 9, 11, 0)),
        Err(StampError::AlreadyOnBreak)
    );
}

#[test]
fn switch_closes_and_continues_with_new_activity() {
    let mut s = StampState::default();
    assert_eq!(
        s.switch(at(30, 9, 0, 0), Some(act(2)), None),
        Err(StampError::NotStampedIn)
    );

    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    assert_eq!(s.switch(at(30, 10, 0, 0), Some(act(2)), None), Ok(1));
    s.stamp_out(at(30, 11, 0, 0)).unwrap();

    let activities: Vec<_> = s.pending.iter().map(|e| e.activity.clone()).collect();
    assert_eq!(activities, vec![Some(act(1)), Some(act(2))]);
}

#[test]
fn session_over_midnight_is_split_per_day() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 22, 0, 0), Some(act(1)), None);
    s.stamp_out(at(31, 2, 0, 0)).unwrap();

    assert_eq!(s.pending.len(), 2);
    assert_eq!(s.pending[0].date, at(30, 0, 0, 0).date());
    assert_eq!(s.pending[0].begin, 22 * 3600);
    assert_eq!(s.pending[0].duration, 2 * 3600);
    assert_eq!(s.pending[1].date, at(31, 0, 0, 0).date());
    assert_eq!(s.pending[1].begin, 0);
    assert_eq!(s.pending[1].duration, 2 * 3600);
}

#[test]
fn zero_length_session_creates_no_entry() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    assert_eq!(s.stamp_out(at(30, 9, 0, 0)), Ok(0));
    assert!(s.pending.is_empty());
}

#[test]
fn unassigned_entries_can_be_assigned_later() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), None, None);
    s.stamp_out(at(30, 10, 0, 0)).unwrap();
    s.stamp_in(at(30, 11, 0, 0), None, None);
    assert_eq!(s.unassigned_count(), 1);

    assert_eq!(s.assign_unassigned(&act(7), at(30, 12, 0, 0)), 2);
    assert_eq!(s.unassigned_count(), 0);
    assert_eq!(s.open.as_ref().unwrap().activity, Some(act(7)));
}

#[test]
fn usage_counts_and_remembers_the_last_use() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    s.switch(at(30, 10, 0, 0), Some(act(2)), None).unwrap();
    s.switch(at(30, 11, 0, 0), Some(act(1)), None).unwrap();
    s.stamp_in(at(30, 12, 0, 0), None, None);

    assert_eq!(s.usage[&1].count, 2);
    assert_eq!(s.usage[&1].last_used, at(30, 11, 0, 0));
    assert_eq!(s.usage[&2].count, 1);
    assert_eq!(
        s.usage.len(),
        2,
        "sessions without activity are not counted"
    );
}

#[test]
fn queued_entry_can_be_removed() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), None, None);
    s.stamp_out(at(30, 10, 0, 0)).unwrap();
    let entry = s.pending[0].clone();

    assert!(s.remove_pending(&entry));
    assert!(s.pending.is_empty());
    assert!(!s.remove_pending(&entry), "already gone");
}

#[test]
fn status_reports_worked_time_without_breaks() {
    let mut s = StampState::default();
    assert_eq!(s.status(at(30, 9, 0, 0)), Status::StampedOut);

    s.stamp_in(at(30, 8, 0, 0), Some(act(1)), None);
    s.start_break(at(30, 9, 0, 0)).unwrap();
    match s.status(at(30, 9, 30, 0)) {
        Status::StampedIn {
            on_break, worked, ..
        } => {
            assert!(on_break);
            assert_eq!(worked, Duration::hours(1));
        }
        other => panic!("unexpected status {other:?}"),
    }
}

#[test]
fn a_forgotten_stamp_out_can_be_ended_in_the_past() {
    let mut s = StampState::default();
    s.stamp_in(at(29, 8, 0, 0), Some(act(1)), Some("report".into()));

    assert_eq!(
        s.stamp_out_at(at(29, 7, 0, 0), at(30, 9, 0, 0)),
        Err(StampError::OutOfRange),
        "before the start"
    );
    assert_eq!(
        s.stamp_out_at(at(30, 10, 0, 0), at(30, 9, 0, 0)),
        Err(StampError::OutOfRange),
        "in the future"
    );
    assert!(s.open.is_some(), "a rejected time changes nothing");

    assert_eq!(s.stamp_out_at(at(29, 17, 30, 0), at(30, 9, 0, 0)), Ok(1));
    assert_eq!(s.pending[0].duration, 9 * 3600 + 1800);
    assert_eq!(s.pending[0].date, at(29, 0, 0, 0).date());
}

#[test]
fn only_long_running_sessions_are_stale() {
    let mut s = StampState::default();
    assert_eq!(s.stale(at(30, 9, 0, 0)), None);
    s.stamp_in(at(29, 21, 0, 0), None, None);
    assert_eq!(s.stale(at(30, 8, 59, 0)), None);
    let stale = s.stale(at(30, 9, 0, 0)).unwrap();
    assert_eq!(stale.since, at(29, 21, 0, 0));
    assert_eq!(stale.elapsed, Duration::hours(12));
}

#[test]
fn a_session_can_be_discarded_without_booking() {
    let mut s = StampState::default();
    assert!(!s.discard_open());
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    assert!(s.discard_open());
    assert!(s.open.is_none() && s.pending.is_empty());
}

#[test]
fn the_report_can_be_set_while_running_and_ends_up_in_the_entry() {
    let mut s = StampState::default();
    assert_eq!(
        s.set_open_comment(Some("x".into())),
        Err(StampError::NotStampedIn)
    );
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), Some("old".into()));
    s.set_open_comment(Some("  fixed login  ".into())).unwrap();
    s.stamp_out(at(30, 10, 0, 0)).unwrap();
    assert_eq!(s.pending[0].comment.as_deref(), Some("fixed login"));
    assert_eq!(s.usage[&1].last_comment.as_deref(), Some("fixed login"));

    s.stamp_in(at(30, 11, 0, 0), Some(act(1)), None);
    s.set_open_comment(Some("   ".into())).unwrap();
    assert_eq!(s.open.as_ref().unwrap().comment, None, "blank removes it");
}

#[test]
fn last_used_is_the_most_recent_activity_with_its_report() {
    let mut s = StampState::default();
    assert_eq!(s.last_used(), None);
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), Some("a".into()));
    s.switch(at(30, 10, 0, 0), Some(act(2)), None).unwrap();
    s.stamp_out(at(30, 11, 0, 0)).unwrap();
    assert_eq!(s.last_used(), Some((2, None)));
    s.stamp_in(at(30, 12, 0, 0), Some(act(1)), None);
    assert_eq!(s.last_used(), Some((1, Some("a"))));
}

#[test]
fn time_away_can_be_taken_off_as_a_past_break() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    s.add_past_break(at(30, 10, 0, 0), at(30, 10, 45, 0), at(30, 11, 0, 0))
        .unwrap();
    s.stamp_out(at(30, 12, 0, 0)).unwrap();
    assert_eq!(s.pending[0].duration, 2 * 3600 + 15 * 60);
    assert_eq!(s.pending[0].pause, 45 * 60);
}

#[test]
fn a_past_break_must_lie_within_the_session() {
    let mut s = StampState::default();
    assert_eq!(
        s.add_past_break(at(30, 10, 0, 0), at(30, 11, 0, 0), at(30, 12, 0, 0)),
        Err(StampError::NotStampedIn)
    );
    s.stamp_in(at(30, 9, 0, 0), None, None);
    let now = at(30, 12, 0, 0);
    for (from, until) in [
        (at(30, 8, 0, 0), at(30, 10, 0, 0)),
        (at(30, 10, 0, 0), at(30, 13, 0, 0)),
        (at(30, 11, 0, 0), at(30, 10, 0, 0)),
    ] {
        assert_eq!(
            s.add_past_break(from, until, now),
            Err(StampError::OutOfRange)
        );
    }
    s.start_break(at(30, 11, 30, 0)).unwrap();
    assert_eq!(
        s.add_past_break(at(30, 10, 0, 0), at(30, 11, 0, 0), now),
        Err(StampError::AlreadyOnBreak)
    );
}

#[test]
fn a_time_of_day_means_its_latest_occurrence() {
    let now = at(30, 9, 30, 0);
    assert_eq!(most_recent_time(9 * 3600, now), at(30, 9, 0, 0));
    assert_eq!(most_recent_time(9 * 3600 + 1800, now), at(30, 9, 30, 0));
    assert_eq!(most_recent_time(17 * 3600, now), at(29, 17, 0, 0));
}

#[test]
fn state_survives_a_save_and_load_round_trip() {
    let path = std::env::temp_dir().join(format!("rolx-stamp-test-{}.json", std::process::id()));
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), Some("fix bug".into()));
    s.stamp_out(at(30, 10, 0, 0)).unwrap();
    s.stamp_in(at(30, 11, 0, 0), None, None);

    s.save_to(&path).unwrap();
    let loaded = StampState::load_from(&path).unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(loaded, s);
}

#[test]
fn missing_file_loads_as_empty_state() {
    let path = std::env::temp_dir().join("rolx-stamp-test-does-not-exist.json");
    assert_eq!(StampState::load_from(&path).unwrap(), StampState::default());
}

#[test]
fn toggle_stamps_out_when_stamped_in() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    let toggled = s.toggle(at(30, 10, 0, 0), |_| panic!("no lookup when stamping out"));
    assert_eq!(toggled, Ok(Toggled::StampedOut));
    assert!(s.open.is_none());
    assert_eq!(s.uploadable_count(), 1);
}

#[test]
fn toggle_stamps_in_with_the_activity_used_last() {
    let mut s = StampState::default();
    s.stamp_in(at(29, 9, 0, 0), Some(act(1)), None);
    s.stamp_in(at(29, 10, 0, 0), Some(act(2)), None);
    s.stamp_out(at(29, 11, 0, 0)).unwrap();

    let toggled = s.toggle(at(30, 8, 0, 0), |id| Some(act(id)));
    assert_eq!(toggled, Ok(Toggled::StampedIn));
    assert_eq!(s.open.as_ref().unwrap().activity, Some(act(2)));
}

#[test]
fn toggle_stamps_in_without_activity_when_it_is_unknown() {
    let mut s = StampState::default();
    assert_eq!(
        s.toggle(at(30, 8, 0, 0), |id| Some(act(id))),
        Ok(Toggled::StampedIn)
    );
    assert_eq!(s.open.as_ref().unwrap().activity, None);

    // Used before, but no longer in the activity list.
    let mut s = StampState::default();
    s.stamp_in(at(29, 9, 0, 0), Some(act(1)), None);
    s.stamp_out(at(29, 10, 0, 0)).unwrap();
    s.toggle(at(30, 8, 0, 0), |_| None).unwrap();
    assert_eq!(s.open.as_ref().unwrap().activity, None);
}

#[test]
fn uploadable_count_skips_unassigned_entries() {
    let mut s = StampState::default();
    s.stamp_in(at(30, 8, 0, 0), None, None);
    s.stamp_in(at(30, 9, 0, 0), Some(act(1)), None);
    s.stamp_out(at(30, 10, 0, 0)).unwrap();
    assert_eq!((s.uploadable_count(), s.unassigned_count()), (1, 1));
}

#[test]
fn update_at_saves_only_when_the_change_succeeds() {
    let path = std::env::temp_dir().join(format!(
        "rolx-stamp-update-test-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let (state, toggled) =
        StampState::update_at(&path, |s| Ok(s.toggle(at(30, 8, 0, 0), |_| None)?)).unwrap();
    assert_eq!(toggled, Toggled::StampedIn);
    assert_eq!(StampState::load_from(&path).unwrap(), state);

    let failed = StampState::update_at(&path, |s| -> anyhow::Result<()> {
        s.discard_open();
        anyhow::bail!("rejected")
    });
    assert!(failed.is_err());
    assert!(StampState::load_from(&path).unwrap().open.is_some());

    std::fs::remove_file(&path).unwrap();
}
