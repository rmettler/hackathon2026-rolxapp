//! Editing the entries of one day: finding overlaps and gaps, changing and fixing entries.
//!
//! Everything here is pure; uploading is done by [`crate::dayedit`]. Times are seconds since midnight,
//! like on the server. An entry covers `begin .. begin + duration + pause`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::stamp::PendingEntry;

pub const SECONDS_PER_DAY: i64 = 86_400;
/// Smaller holes between entries are not reported; stamps and manual edits are never that exact.
pub const MIN_GAP_SECS: i64 = 60;

/// `08:05` for a time of day in seconds.
pub fn fmt_clock(secs: i64) -> String {
    format!("{:02}:{:02}", secs / 3600, secs / 60 % 60)
}

/// `1:30` for a length in seconds.
pub fn fmt_hm(secs: i64) -> String {
    format!("{}:{:02}", secs / 3600, secs / 60 % 60)
}

/// Parses `H:MM` or `HH:MM` into seconds since midnight (`24:00` is allowed as the end of the day).
pub fn parse_clock(text: &str) -> Option<i32> {
    let (hours, minutes) = text.trim().split_once(':')?;
    let hours: i32 = hours.parse().ok()?;
    let minutes: i32 = minutes.parse().ok()?;
    let secs = hours * 3600 + minutes * 60;
    ((0..60).contains(&minutes) && (0..=SECONDS_PER_DAY as i32).contains(&secs)).then_some(secs)
}

/// An entry of a day record, as the server sends and expects it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayEntry {
    pub activity_id: i32,
    #[serde(default)]
    pub full_activity_number: String,
    #[serde(default)]
    pub begin: Option<i32>,
    pub duration: i64,
    #[serde(default)]
    pub pause: Option<i32>,
    #[serde(default)]
    pub comment: Option<String>,
}

impl DayEntry {
    /// End of the covered time span, if the entry has a time of day.
    pub fn end(&self) -> Option<i64> {
        self.begin
            .map(|b| i64::from(b) + self.duration + i64::from(self.pause.unwrap_or(0)))
    }
}

/// A queued entry as it will look on the server (activity id 0 while unassigned).
impl From<&PendingEntry> for DayEntry {
    fn from(p: &PendingEntry) -> Self {
        DayEntry {
            activity_id: p.activity.as_ref().map_or(0, |a| a.id),
            full_activity_number: p
                .activity
                .as_ref()
                .map_or_else(String::new, |a| a.full_number.clone()),
            begin: Some(p.begin),
            duration: p.duration,
            pause: Some(p.pause).filter(|p| *p > 0),
            comment: p.comment.clone(),
        }
    }
}

/// A problem between two entries. Indices refer to the slice that was analyzed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// `second` starts `secs` before `first` (the entry reaching furthest so far) has ended.
    Overlap {
        first: usize,
        second: usize,
        secs: i64,
    },
    /// Nothing is booked for `secs` between `first` and `second`.
    Gap {
        first: usize,
        second: usize,
        secs: i64,
    },
}

impl Issue {
    pub fn is_overlap(&self) -> bool {
        matches!(self, Issue::Overlap { .. })
    }

    pub fn indices(&self) -> (usize, usize) {
        match *self {
            Issue::Overlap { first, second, .. } | Issue::Gap { first, second, .. } => {
                (first, second)
            }
        }
    }

    /// A one-line description; `label` names an entry by index (e.g. `#2`).
    pub fn describe(&self, label: &dyn Fn(usize) -> String) -> String {
        match *self {
            Issue::Overlap {
                first,
                second,
                secs,
            } => format!(
                "{} and {} overlap by {}",
                label(first),
                label(second),
                fmt_hm(secs)
            ),
            Issue::Gap {
                first,
                second,
                secs,
            } => format!(
                "{} unbooked between {} and {}",
                fmt_hm(secs),
                label(first),
                label(second)
            ),
        }
    }

    /// Identifies the issue by the entries involved, so it can be recognized after indices moved.
    fn key(&self, entries: &[DayEntry]) -> (bool, DayEntry, DayEntry) {
        let (first, second) = self.indices();
        (
            self.is_overlap(),
            entries[first].clone(),
            entries[second].clone(),
        )
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("there is no such entry")]
    NoSuchEntry,
    #[error("the entry has no time of day, so begin and end cannot be changed relative to it")]
    NoBeginTime,
    #[error("the end must be after the begin")]
    InvalidTimes,
    #[error("the entry must stay within the day")]
    OutsideDay,
    #[error("the break is longer than the entry")]
    BreakLongerThanEntry,
    #[error("this fix does not apply to that kind of problem")]
    FixNotApplicable,
}

/// Finds overlaps and gaps between the entries that have a time of day.
pub fn analyze(entries: &[DayEntry]) -> Vec<Issue> {
    let mut timed: Vec<usize> = (0..entries.len())
        .filter(|&i| entries[i].begin.is_some() && entries[i].duration > 0)
        .collect();
    timed.sort_by_key(|&i| (entries[i].begin, i));

    let mut issues = Vec::new();
    // The entry that reaches furthest so far, so an entry nested in a longer one is handled.
    let mut reach: Option<(usize, i64)> = None;
    for &i in &timed {
        let begin = i64::from(entries[i].begin.unwrap_or(0));
        let end = entries[i].end().unwrap_or(begin);
        match reach {
            Some((first, reach_end)) => {
                if begin < reach_end {
                    issues.push(Issue::Overlap {
                        first,
                        second: i,
                        secs: reach_end - begin,
                    });
                } else if begin - reach_end >= MIN_GAP_SECS {
                    issues.push(Issue::Gap {
                        first,
                        second: i,
                        secs: begin - reach_end,
                    });
                }
                if end > reach_end {
                    reach = Some((i, end));
                }
            }
            None => reach = Some((i, end)),
        }
    }
    issues
}

/// Issues in `after` that did not exist in `before` (compared by the entries involved).
pub fn new_issues(before: &[DayEntry], after: &[DayEntry]) -> Vec<Issue> {
    let known: Vec<_> = analyze(before).iter().map(|i| i.key(before)).collect();
    analyze(after)
        .into_iter()
        .filter(|i| !known.contains(&i.key(after)))
        .collect()
}

/// A change to one entry. `None` leaves a field as it is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntryChange {
    pub activity: Option<(i32, String)>,
    /// `Some(None)` removes the comment.
    pub comment: Option<Option<String>>,
    pub begin: Option<i32>,
    pub end: Option<i32>,
}

/// Applies the change to `entries[index]`. Changing begin or end keeps the pause and adjusts the
/// duration.
pub fn apply_change(
    entries: &mut [DayEntry],
    index: usize,
    change: &EntryChange,
) -> Result<(), EditError> {
    let entry = entries.get_mut(index).ok_or(EditError::NoSuchEntry)?;
    let mut updated = entry.clone();

    if let Some((id, number)) = &change.activity {
        updated.activity_id = *id;
        updated.full_activity_number = number.clone();
    }
    if let Some(comment) = &change.comment {
        updated.comment = comment.clone().filter(|c| !c.trim().is_empty());
    }
    if change.begin.is_some() || change.end.is_some() {
        let begin = i64::from(change.begin.or(entry.begin).ok_or(EditError::NoBeginTime)?);
        let end = match change.end {
            Some(end) => i64::from(end),
            None => entry.end().ok_or(EditError::NoBeginTime)?,
        };
        if begin < 0 || end <= begin {
            return Err(EditError::InvalidTimes);
        }
        if end > SECONDS_PER_DAY {
            return Err(EditError::OutsideDay);
        }
        let pause = i64::from(entry.pause.unwrap_or(0));
        if pause >= end - begin {
            return Err(EditError::BreakLongerThanEntry);
        }
        updated.begin = Some(begin as i32);
        updated.duration = end - begin - pause;
    }

    *entry = updated;
    Ok(())
}

/// Ways to resolve an [`Issue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fix {
    /// Overlap: end the earlier entry where the later one starts.
    TruncateFirst,
    /// Overlap: move the later entry back so it starts when the earlier one ends.
    ShiftSecond,
    /// Gap: count the unbooked time as a break of the earlier entry.
    FillAsBreak,
}

impl Fix {
    fn handles_overlaps(self) -> bool {
        matches!(self, Fix::TruncateFirst | Fix::ShiftSecond)
    }
}

/// Applies one fix. The entries are only changed if it succeeds.
pub fn apply_fix(entries: &mut Vec<DayEntry>, issue: &Issue, fix: Fix) -> Result<(), EditError> {
    if issue.is_overlap() != fix.handles_overlaps() {
        return Err(EditError::FixNotApplicable);
    }
    let (first, second) = issue.indices();
    if first >= entries.len() || second >= entries.len() {
        return Err(EditError::NoSuchEntry);
    }
    let first_end = entries[first].end().ok_or(EditError::NoBeginTime)?;
    let second_begin = i64::from(entries[second].begin.ok_or(EditError::NoBeginTime)?);

    match fix {
        Fix::TruncateFirst => {
            let first_begin = i64::from(entries[first].begin.ok_or(EditError::NoBeginTime)?);
            let span = second_begin - first_begin;
            let pause = i64::from(entries[first].pause.unwrap_or(0)).min(span.max(0));
            let duration = span - pause;
            if duration <= 0 {
                entries.remove(first);
            } else {
                entries[first].duration = duration;
                entries[first].pause = Some(pause as i32).filter(|p| *p > 0);
            }
        }
        Fix::ShiftSecond => {
            let moved = &mut entries[second];
            let new_end = first_end + moved.duration + i64::from(moved.pause.unwrap_or(0));
            if new_end > SECONDS_PER_DAY {
                return Err(EditError::OutsideDay);
            }
            moved.begin = Some(first_end as i32);
        }
        Fix::FillAsBreak => {
            let gap = second_begin - first_end;
            if gap > 0 {
                let pause = i64::from(entries[first].pause.unwrap_or(0)) + gap;
                entries[first].pause = Some(pause as i32);
            }
        }
    }
    Ok(())
}

/// Repeatedly applies `fix` to the problems of its kind that `entries` has but `before` did not.
/// This is how an edit that created a conflict gets resolved in one go.
pub fn fix_new_issues(
    entries: &mut Vec<DayEntry>,
    before: &[DayEntry],
    fix: Fix,
) -> Result<(), EditError> {
    // A fix can uncover the next conflict (shifting pushes into the following entry).
    for _ in 0..(2 * entries.len() + 2) {
        let next = new_issues(before, entries)
            .into_iter()
            .find(|i| i.is_overlap() == fix.handles_overlaps());
        match next {
            Some(issue) => apply_fix(entries, &issue, fix)?,
            None => return Ok(()),
        }
    }
    Ok(())
}

/// The entries of a record as returned by the server.
pub fn entries_of(record: &Value) -> Result<Vec<DayEntry>, serde_json::Error> {
    match record.get("entries") {
        Some(list) => serde_json::from_value(list.clone()),
        None => Ok(Vec::new()),
    }
}

/// Replaces the entries of a record, leaving all other fields untouched.
pub fn set_entries(record: &mut Value, entries: &[DayEntry]) {
    if let Some(object) = record.as_object_mut() {
        object.insert(
            "entries".into(),
            serde_json::to_value(entries).unwrap_or(Value::Null),
        );
    }
}

#[cfg(test)]
#[path = "dayplan.tests.rs"]
mod tests;
