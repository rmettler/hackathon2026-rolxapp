//! The data the fake server starts with, relative to the day it is started.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};

use super::{Activity, Day, Entry, State};

/// Builds the activities (one closed last Friday), favourites, an edit lock at the Monday of last week, and entries
/// for last week and the current week up to `today`.
pub fn state(today: NaiveDate) -> State {
    let monday = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let last_friday = monday - Duration::days(3);

    #[rustfmt::skip]
    let activities = vec![
        act(1, (1001, 1, 1), ("Example Engineering AG", "RolX", "Development", "Implementation"), true, None),
        act(2, (1001, 1, 2), ("Example Engineering AG", "RolX", "Development", "Code Review"), true, None),
        act(3, (1001, 2, 1), ("Example Engineering AG", "RolX", "Support", "Bugfixing"), true, None),
        act(4, (2042, 1, 1), ("Stadtwerke Musterhausen", "Smart Metering", "Phase 1", "Requirements"), true, None),
        act(5, (2042, 1, 2), ("Stadtwerke Musterhausen", "Smart Metering", "Phase 1", "Implementation"), true, None),
        act(6, (2042, 2, 1), ("Stadtwerke Musterhausen", "Smart Metering", "Phase 2", "Rollout"), true, None),
        act(7, (3107, 1, 1), ("Alpenbahn AG", "Ticketing App", "Mobile", "Android"), true, None),
        act(8, (3107, 1, 2), ("Alpenbahn AG", "Ticketing App", "Mobile", "iOS"), true, None),
        // Closed: only listed as "suitable" (booked last week), the server rejects new bookings.
        act(9, (3107, 1, 3), ("Alpenbahn AG", "Ticketing App", "Mobile", "Beta Testing"), true, Some(last_friday)),
        act(10, (1, 1, 1), ("Internal", "Administration", "General", "Meetings"), false, None),
        act(11, (1, 1, 2), ("Internal", "Administration", "General", "Training"), false, None),
        act(12, (1, 2, 1), ("Internal", "Innovation", "Hackathon", "Hackathon 2026"), false, None),
    ];
    for a in &activities {
        assert!(a.end.is_none_or(|end| end >= a.start));
    }

    // (weekday offset from Monday, entries)
    #[rustfmt::skip]
    let last_week: [(i64, Vec<Entry>); 5] = [
        (0, vec![e(5, "08:00", 240, 0, "Zählerdaten importieren"), e(10, "12:00", 30, 0, "Weekly"), e(5, "13:00", 210, 0, "")]),
        (1, vec![e(4, "08:15", 180, 0, "Workshop mit Kunde"), e(5, "12:30", 285, 15, "")]),
        (2, vec![e(7, "08:00", 255, 0, "Offline-Modus"), e(7, "13:00", 249, 0, "")]),
        (3, vec![e(9, "08:00", 240, 0, "Testplan Beta"), e(3, "12:45", 264, 0, "Ticket #4711")]),
        (4, vec![e(9, "08:00", 180, 0, "Beta-Feedback auswerten"), e(12, "11:00", 60, 0, "Ideen sammeln"), e(1, "13:00", 264, 0, "")]),
    ];
    #[rustfmt::skip]
    let this_week: [(i64, Vec<Entry>); 5] = [
        (0, vec![e(1, "08:00", 240, 0, "Login-Flow Android"), e(5, "13:00", 264, 0, "")]),
        (1, vec![e(5, "07:45", 210, 0, ""), e(10, "11:15", 30, 0, "Daily"), e(5, "12:30", 255, 15, "Schnittstelle MDM")]),
        // An entry without a time of day, which the real server allows.
        (2, vec![e(2, "08:30", 120, 0, "Review Sync"), e(10, "10:30", 60, 0, ""), e(7, "13:00", 210, 0, ""), duration_only(11, 60, "Rust-Kurs")]),
        (3, vec![e(7, "08:00", 240, 0, "Push-Benachrichtigungen"), e(1, "12:45", 264, 0, "")]),
        (4, vec![e(1, "08:00", 120, 0, "Fake-Server")]),
    ];

    let mut days = BTreeMap::new();
    for (offset, entries) in last_week {
        days.insert(monday - Duration::days(7 - offset), day(entries));
    }
    for (offset, entries) in this_week {
        let date = monday + Duration::days(offset);
        // Today only gets the morning, so there is room to stamp and book.
        if date < today {
            days.insert(date, day(entries));
        } else if date == today {
            days.insert(date, day(entries.into_iter().take(1).collect()));
        }
    }

    State {
        activities,
        favourites: BTreeSet::from([1, 5, 10]),
        days,
        edit_lock: monday - Duration::days(7),
    }
}

fn act(
    id: i32,
    (project_number, subproject_number, number): (u32, u32, u32),
    (customer, project, subproject, name): (&str, &str, &str, &str),
    billable: bool,
    end: Option<NaiveDate>,
) -> Activity {
    Activity {
        id,
        number,
        name: name.into(),
        project_number,
        subproject_number,
        customer: customer.into(),
        project: project.into(),
        subproject: subproject.into(),
        billable,
        start: NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"),
        end,
        budget: if billable { 200 * 3600 } else { 0 },
    }
}

fn day(entries: Vec<Entry>) -> Day {
    Day {
        paid_leave_type: None,
        paid_leave_reason: None,
        entries,
    }
}

/// An entry from `begin` (HH:MM) lasting `minutes` of work plus `pause_minutes` of break.
fn e(activity_id: i32, begin: &str, minutes: i64, pause_minutes: i64, comment: &str) -> Entry {
    let (h, m) = begin.split_once(':').expect("HH:MM");
    let begin = h.parse::<i64>().expect("hours") * 3600 + m.parse::<i64>().expect("minutes") * 60;
    Entry {
        activity_id,
        duration: minutes * 60,
        begin: Some(begin),
        pause: (pause_minutes > 0).then_some(pause_minutes * 60),
        comment: comment.into(),
    }
}

fn duration_only(activity_id: i32, minutes: i64, comment: &str) -> Entry {
    Entry {
        begin: None,
        ..e(activity_id, "00:00", minutes, 0, comment)
    }
}
