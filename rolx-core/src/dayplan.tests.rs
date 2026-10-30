use serde_json::json;

use super::*;

/// An entry from `h:mm` for `minutes`, with `pause` minutes of break.
fn e(h: i32, m: i32, minutes: i64, pause: i32) -> DayEntry {
    DayEntry {
        activity_id: 1,
        full_activity_number: "A-1".into(),
        begin: Some(h * 3600 + m * 60),
        duration: minutes * 60,
        pause: Some(pause * 60).filter(|p| *p > 0),
        comment: None,
    }
}

fn at(h: i32, m: i32) -> Option<i32> {
    Some(h * 3600 + m * 60)
}

#[test]
fn back_to_back_entries_have_no_issues() {
    let day = vec![e(8, 0, 120, 0), e(10, 0, 60, 0), e(11, 0, 30, 0)];
    assert!(analyze(&day).is_empty());
}

#[test]
fn detects_overlap_and_reports_its_length() {
    let day = vec![e(8, 0, 120, 0), e(9, 30, 60, 0)];
    assert_eq!(
        analyze(&day),
        vec![Issue::Overlap {
            first: 0,
            second: 1,
            secs: 30 * 60
        }]
    );
}

#[test]
fn pause_counts_as_part_of_the_entry() {
    // 8:00 + 2h work + 30min break ends 10:30, so a 10:15 start overlaps.
    let day = vec![e(8, 0, 120, 30), e(10, 15, 30, 0)];
    assert_eq!(
        analyze(&day),
        vec![Issue::Overlap {
            first: 0,
            second: 1,
            secs: 15 * 60
        }]
    );
}

#[test]
fn detects_gaps_but_not_tiny_ones() {
    let day = vec![e(8, 0, 60, 0), e(10, 15, 60, 0)];
    assert_eq!(
        analyze(&day),
        vec![Issue::Gap {
            first: 0,
            second: 1,
            secs: 75 * 60
        }]
    );

    let mut nearly = vec![e(8, 0, 60, 0), e(9, 0, 60, 0)];
    nearly[1].begin = Some(9 * 3600 + 30);
    assert!(
        analyze(&nearly).is_empty(),
        "30 s hole is below the threshold"
    );
}

#[test]
fn nested_entry_does_not_hide_a_later_overlap() {
    // Long entry 8:00-12:00, short one inside it, then one at 11:00 that still overlaps the long one.
    let day = vec![e(8, 0, 240, 0), e(9, 0, 30, 0), e(11, 0, 60, 0)];
    let issues = analyze(&day);
    assert_eq!(issues.len(), 2);
    assert_eq!(
        issues[1],
        Issue::Overlap {
            first: 0,
            second: 2,
            secs: 60 * 60
        }
    );
}

#[test]
fn entries_without_time_of_day_are_ignored() {
    let mut untimed = e(9, 0, 60, 0);
    untimed.begin = None;
    let day = vec![e(8, 0, 120, 0), untimed];
    assert!(analyze(&day).is_empty());
}

#[test]
fn only_new_issues_are_reported_after_an_edit() {
    let before = vec![e(8, 0, 60, 0), e(9, 0, 60, 0), e(14, 0, 60, 0)]; // gap 10:00-14:00
    let mut after = before.clone();
    apply_change(
        &mut after,
        0,
        &EntryChange {
            end: at(9, 30),
            ..Default::default()
        },
    )
    .unwrap();

    let new = new_issues(&before, &after);
    assert_eq!(new.len(), 1);
    assert!(new[0].is_overlap(), "the old gap is not new: {new:?}");
}

#[test]
fn change_of_times_keeps_the_pause_and_recomputes_the_duration() {
    let mut day = vec![e(8, 0, 120, 30)]; // 8:00-10:30
    apply_change(
        &mut day,
        0,
        &EntryChange {
            end: at(12, 0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(day[0].begin, at(8, 0));
    assert_eq!(day[0].pause, Some(30 * 60));
    assert_eq!(day[0].duration, (240 - 30) * 60);

    apply_change(
        &mut day,
        0,
        &EntryChange {
            begin: at(9, 0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(day[0].end(), Some(12 * 3600));
}

#[test]
fn change_of_activity_and_comment() {
    let mut day = vec![e(8, 0, 60, 0)];
    apply_change(
        &mut day,
        0,
        &EntryChange {
            activity: Some((9, "B-9".into())),
            comment: Some(Some("fixed it".into())),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(day[0].activity_id, 9);
    assert_eq!(day[0].full_activity_number, "B-9");
    assert_eq!(day[0].comment.as_deref(), Some("fixed it"));

    apply_change(
        &mut day,
        0,
        &EntryChange {
            comment: Some(Some("   ".into())),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(day[0].comment, None, "a blank comment is removed");
}

#[test]
fn invalid_changes_are_rejected_and_leave_the_entry_alone() {
    let original = e(8, 0, 120, 30);
    let mut day = vec![original.clone()];
    let change = |c: EntryChange| apply_change(&mut [original.clone()], 0, &c);

    assert_eq!(
        change(EntryChange {
            end: at(7, 0),
            ..Default::default()
        }),
        Err(EditError::InvalidTimes)
    );
    assert_eq!(
        change(EntryChange {
            end: Some(86_400 + 60),
            ..Default::default()
        }),
        Err(EditError::OutsideDay)
    );
    assert_eq!(
        change(EntryChange {
            end: at(8, 20),
            ..Default::default()
        }),
        Err(EditError::BreakLongerThanEntry)
    );
    assert_eq!(
        apply_change(&mut day, 3, &EntryChange::default()),
        Err(EditError::NoSuchEntry)
    );

    let mut untimed = vec![DayEntry {
        begin: None,
        ..original.clone()
    }];
    assert_eq!(
        apply_change(
            &mut untimed,
            0,
            &EntryChange {
                end: at(9, 0),
                ..Default::default()
            }
        ),
        Err(EditError::NoBeginTime)
    );
    assert_eq!(day, vec![original]);
}

#[test]
fn changing_the_break_keeps_the_time_span_and_adjusts_the_work_time() {
    let mut day = vec![e(8, 0, 240, 0)]; // 8:00-12:00, no break
    apply_change(
        &mut day,
        0,
        &EntryChange {
            pause: Some(45 * 60),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(day[0].end(), Some(12 * 3600), "span unchanged");
    assert_eq!(day[0].pause, Some(45 * 60));
    assert_eq!(day[0].duration, (240 - 45) * 60);

    apply_change(
        &mut day,
        0,
        &EntryChange {
            pause: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(day[0].pause, None, "a zero break is removed");
    assert_eq!(day[0].duration, 240 * 60);
}

#[test]
fn break_and_times_can_be_changed_together() {
    let mut day = vec![e(8, 0, 240, 0)];
    apply_change(
        &mut day,
        0,
        &EntryChange {
            end: at(13, 0),
            pause: Some(60 * 60),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(day[0].begin, at(8, 0));
    assert_eq!(day[0].duration, 4 * 3600, "5h span minus 1h break");
    assert_eq!(day[0].pause, Some(3600));
}

#[test]
fn invalid_breaks_are_rejected() {
    let mut day = vec![e(8, 0, 60, 0)];
    let too_long = EntryChange {
        pause: Some(60 * 60),
        ..Default::default()
    };
    assert_eq!(
        apply_change(&mut day, 0, &too_long),
        Err(EditError::BreakLongerThanEntry)
    );
    let negative = EntryChange {
        pause: Some(-1),
        ..Default::default()
    };
    assert_eq!(
        apply_change(&mut day, 0, &negative),
        Err(EditError::InvalidTimes)
    );

    let mut untimed = vec![DayEntry {
        begin: None,
        ..e(8, 0, 60, 0)
    }];
    let pause_only = EntryChange {
        pause: Some(600),
        ..Default::default()
    };
    assert_eq!(
        apply_change(&mut untimed, 0, &pause_only),
        Err(EditError::NoBeginTime),
        "the server only allows a break on entries with a time of day"
    );
    assert_eq!(
        day[0].duration, 3600,
        "failed changes leave the entry alone"
    );
}

#[test]
fn truncate_ends_the_earlier_entry_where_the_later_starts() {
    let mut day = vec![e(8, 0, 120, 0), e(9, 30, 60, 0)];
    let issue = analyze(&day)[0];
    apply_fix(&mut day, &issue, Fix::TruncateFirst).unwrap();

    assert_eq!(day[0].duration, 90 * 60);
    assert!(analyze(&day).is_empty());
}

#[test]
fn truncate_drops_an_entry_that_would_become_empty() {
    let mut day = vec![e(9, 0, 60, 0), e(9, 0, 30, 0)];
    let issue = analyze(&day)[0];
    apply_fix(&mut day, &issue, Fix::TruncateFirst).unwrap();
    assert_eq!(day.len(), 1);
}

#[test]
fn shift_moves_the_later_entry_to_the_end_of_the_earlier() {
    let mut day = vec![e(8, 0, 120, 0), e(9, 30, 60, 0)];
    let issue = analyze(&day)[0];
    apply_fix(&mut day, &issue, Fix::ShiftSecond).unwrap();

    assert_eq!(day[1].begin, at(10, 0));
    assert_eq!(day[1].duration, 60 * 60, "work time is kept");
    assert!(analyze(&day).is_empty());
}

#[test]
fn shift_beyond_midnight_fails_and_changes_nothing() {
    let mut day = vec![e(22, 0, 120, 0), e(23, 0, 60, 0)];
    let before = day.clone();
    let issue = analyze(&day)[0];
    assert_eq!(
        apply_fix(&mut day, &issue, Fix::ShiftSecond),
        Err(EditError::OutsideDay)
    );
    assert_eq!(day, before);
}

#[test]
fn fill_as_break_closes_a_gap_by_extending_the_pause() {
    let mut day = vec![e(8, 0, 60, 0), e(10, 0, 60, 0)];
    let issue = analyze(&day)[0];
    apply_fix(&mut day, &issue, Fix::FillAsBreak).unwrap();

    assert_eq!(day[0].pause, Some(60 * 60));
    assert_eq!(day[0].duration, 60 * 60, "no extra work time is booked");
    assert!(analyze(&day).is_empty());
}

#[test]
fn fix_must_match_the_kind_of_issue() {
    let mut gap = vec![e(8, 0, 60, 0), e(10, 0, 60, 0)];
    let issue = analyze(&gap)[0];
    assert_eq!(
        apply_fix(&mut gap, &issue, Fix::TruncateFirst),
        Err(EditError::FixNotApplicable)
    );

    let mut overlap = vec![e(8, 0, 120, 0), e(9, 0, 60, 0)];
    let issue = analyze(&overlap)[0];
    assert_eq!(
        apply_fix(&mut overlap, &issue, Fix::FillAsBreak),
        Err(EditError::FixNotApplicable)
    );
}

#[test]
fn shifting_cascades_through_following_entries() {
    let before = vec![e(8, 0, 60, 0), e(9, 0, 60, 0), e(10, 0, 60, 0)];
    let mut after = before.clone();
    apply_change(
        &mut after,
        0,
        &EntryChange {
            end: at(9, 30),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!new_issues(&before, &after).is_empty());

    fix_new_issues(&mut after, &before, Fix::ShiftSecond).unwrap();

    assert_eq!(after[1].begin, at(9, 30));
    assert_eq!(after[2].begin, at(10, 30));
    assert!(analyze(&after).is_empty());
}

#[test]
fn entries_are_sorted_by_time_with_untimed_entries_last() {
    let untimed = DayEntry {
        begin: None,
        ..e(9, 0, 60, 0)
    };
    let mut day = vec![
        untimed.clone(),
        e(13, 0, 60, 0),
        e(8, 0, 120, 0),
        e(8, 0, 60, 0),
    ];
    sort_entries(&mut day);

    assert_eq!(
        day,
        vec![
            e(8, 0, 60, 0), // same start: the one that ends first
            e(8, 0, 120, 0),
            e(13, 0, 60, 0),
            untimed,
        ]
    );
}

#[test]
fn sorting_gives_the_same_result_whatever_the_input_order() {
    let items = vec![
        e(10, 0, 30, 0),
        DayEntry {
            activity_id: 2,
            ..e(10, 0, 30, 0)
        },
        DayEntry {
            begin: None,
            ..e(0, 0, 60, 0)
        },
        e(9, 0, 30, 0),
        DayEntry {
            comment: Some("x".into()),
            ..e(10, 0, 30, 0)
        },
    ];
    let mut forward = items.clone();
    let mut backward: Vec<_> = items.into_iter().rev().collect();
    sort_entries(&mut forward);
    sort_entries(&mut backward);
    assert_eq!(forward, backward);
}

#[test]
fn clock_times_are_parsed_and_formatted() {
    assert_eq!(parse_clock("9:05"), Some(9 * 3600 + 5 * 60));
    assert_eq!(parse_clock(" 08:30 "), Some(8 * 3600 + 30 * 60));
    assert_eq!(parse_clock("24:00"), Some(86_400));
    assert_eq!(parse_clock("24:01"), None);
    assert_eq!(parse_clock("12:60"), None);
    assert_eq!(parse_clock("noon"), None);
    assert_eq!(parse_clock("1230"), None);

    assert_eq!(fmt_clock(9 * 3600 + 5 * 60), "09:05");
    assert_eq!(fmt_hm(90 * 60), "1:30");
}

#[test]
fn queued_entries_convert_to_day_entries() {
    use crate::stamp::{ActivityRef, PendingEntry};
    let queued = PendingEntry {
        date: chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap(),
        begin: 3600,
        duration: 1800,
        pause: 300,
        activity: Some(ActivityRef {
            id: 4,
            full_number: "A-4".into(),
        }),
        comment: Some("c".into()),
    };
    let entry = DayEntry::from(&queued);
    assert_eq!(entry.activity_id, 4);
    assert_eq!(entry.pause, Some(300));
    assert_eq!(entry.end(), Some(3600 + 1800 + 300));
}

#[test]
fn record_entries_round_trip_without_touching_other_fields() {
    let mut record = json!({
        "date": "2026-10-30",
        "userId": "u1",
        "dayType": 0,
        "entries": [
            {"activityId": 5, "fullActivityNumber": "A-5", "duration": 1800, "begin": 28800, "pause": null, "comment": "x"}
        ]
    });
    let mut entries = entries_of(&record).unwrap();
    assert_eq!(entries[0].activity_id, 5);
    assert_eq!(entries[0].pause, None);

    entries[0].duration = 3600;
    set_entries(&mut record, &entries);

    assert_eq!(record["entries"][0]["duration"], 3600);
    assert_eq!(record["entries"][0]["comment"], "x");
    assert_eq!(record["dayType"], 0);
    assert_eq!(record["userId"], "u1");
}
