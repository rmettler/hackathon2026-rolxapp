//! Offline-first stamping: work events are recorded in a local file and uploaded later by [`crate::sync`].
//!
//! Times are local wall-clock times, matching how the server stores `begin` (seconds since midnight).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

use crate::storage;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityRef {
    pub id: i32,
    pub full_number: String,
}

/// How often and when an activity was last booked with the stamp functions. Drives "recent".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub count: u32,
    pub last_used: NaiveDateTime,
}

/// The work session that is currently running.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenStamp {
    pub start: NaiveDateTime,
    pub activity: Option<ActivityRef>,
    pub comment: Option<String>,
    pub break_started: Option<NaiveDateTime>,
    /// Total length of the finished breaks of this session.
    pub paused_secs: i64,
}

/// A finished work entry that still has to be uploaded. Mirrors the server's `RecordEntry`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingEntry {
    pub date: NaiveDate,
    /// Seconds since midnight.
    pub begin: i32,
    /// Net work time in seconds (without the pause).
    pub duration: i64,
    /// Seconds of breaks within the entry.
    pub pause: i32,
    /// `None` means "unassigned": it is kept locally until an activity is assigned.
    pub activity: Option<ActivityRef>,
    pub comment: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StampState {
    pub open: Option<OpenStamp>,
    pub pending: Vec<PendingEntry>,
    /// Keyed by activity id.
    pub usage: BTreeMap<i32, Usage>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    StampedOut,
    StampedIn {
        since: NaiveDateTime,
        activity: Option<ActivityRef>,
        on_break: bool,
        /// Time worked in this session so far, excluding breaks.
        worked: Duration,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StampError {
    #[error("not stamped in")]
    NotStampedIn,
    #[error("already on a break")]
    AlreadyOnBreak,
    #[error("not on a break")]
    NotOnBreak,
}

fn whole_seconds(t: NaiveDateTime) -> NaiveDateTime {
    t.with_nanosecond(0).unwrap_or(t)
}

impl OpenStamp {
    fn new(start: NaiveDateTime, activity: Option<ActivityRef>, comment: Option<String>) -> Self {
        Self {
            start,
            activity,
            comment,
            break_started: None,
            paused_secs: 0,
        }
    }

    /// Finishes the session, ending a running break. Entries never span midnight because the
    /// server requires each entry to end within its day.
    fn close(mut self, end: NaiveDateTime) -> Vec<PendingEntry> {
        let end = end.max(self.start);
        if let Some(break_start) = self.break_started.take() {
            self.paused_secs += (end - break_start).num_seconds().max(0);
        }

        let mut remaining_pause = self.paused_secs;
        let mut entries = Vec::new();
        let mut segment_start = self.start;
        while segment_start < end {
            let next_midnight = segment_start
                .date()
                .succ_opt()
                .map_or(end, |d| d.and_time(NaiveTime::MIN));
            let segment_end = end.min(next_midnight);
            let span = (segment_end - segment_start).num_seconds();
            let pause = remaining_pause.min(span);
            remaining_pause -= pause;
            let duration = span - pause;
            if duration > 0 {
                entries.push(PendingEntry {
                    date: segment_start.date(),
                    begin: segment_start.time().num_seconds_from_midnight() as i32,
                    duration,
                    pause: pause as i32,
                    activity: self.activity.clone(),
                    comment: self.comment.clone(),
                });
            }
            segment_start = segment_end;
        }
        entries
    }
}

impl StampState {
    /// Starts a session. A session that is still running is closed at `now` first.
    /// Returns how many entries that auto-close produced.
    pub fn stamp_in(
        &mut self,
        now: NaiveDateTime,
        activity: Option<ActivityRef>,
        comment: Option<String>,
    ) -> usize {
        let now = whole_seconds(now);
        let closed = self.close_open(now);
        if let Some(a) = &activity {
            self.touch(a.id, now);
        }
        self.open = Some(OpenStamp::new(now, activity, comment));
        closed
    }

    /// Ends the session (ending a running break first). Returns the number of entries produced.
    pub fn stamp_out(&mut self, now: NaiveDateTime) -> Result<usize, StampError> {
        if self.open.is_none() {
            return Err(StampError::NotStampedIn);
        }
        Ok(self.close_open(whole_seconds(now)))
    }

    pub fn start_break(&mut self, now: NaiveDateTime) -> Result<(), StampError> {
        let open = self.open.as_mut().ok_or(StampError::NotStampedIn)?;
        if open.break_started.is_some() {
            return Err(StampError::AlreadyOnBreak);
        }
        open.break_started = Some(whole_seconds(now).max(open.start));
        Ok(())
    }

    pub fn end_break(&mut self, now: NaiveDateTime) -> Result<(), StampError> {
        let open = self.open.as_mut().ok_or(StampError::NotStampedIn)?;
        let break_start = open.break_started.take().ok_or(StampError::NotOnBreak)?;
        open.paused_secs += (whole_seconds(now) - break_start).num_seconds().max(0);
        Ok(())
    }

    /// Continues with another activity: closes the current session and starts a new one right away.
    pub fn switch(
        &mut self,
        now: NaiveDateTime,
        activity: Option<ActivityRef>,
        comment: Option<String>,
    ) -> Result<usize, StampError> {
        if self.open.is_none() {
            return Err(StampError::NotStampedIn);
        }
        Ok(self.stamp_in(now, activity, comment))
    }

    /// Assigns the activity to everything that has none yet (queued entries and the running
    /// session). Returns how many items were changed.
    pub fn assign_unassigned(&mut self, activity: &ActivityRef, now: NaiveDateTime) -> usize {
        let mut changed = 0;
        for entry in self.pending.iter_mut().filter(|e| e.activity.is_none()) {
            entry.activity = Some(activity.clone());
            changed += 1;
        }
        if let Some(open) = self.open.as_mut().filter(|o| o.activity.is_none()) {
            open.activity = Some(activity.clone());
            changed += 1;
        }
        if changed > 0 {
            self.touch(activity.id, whole_seconds(now));
        }
        changed
    }

    /// Removes a queued entry that was not uploaded yet. Returns whether it was found.
    pub fn remove_pending(&mut self, entry: &PendingEntry) -> bool {
        match self.pending.iter().position(|e| e == entry) {
            Some(i) => {
                self.pending.remove(i);
                true
            }
            None => false,
        }
    }

    fn touch(&mut self, activity_id: i32, now: NaiveDateTime) {
        let usage = self.usage.entry(activity_id).or_insert(Usage {
            count: 0,
            last_used: now,
        });
        usage.count += 1;
        usage.last_used = now;
    }

    pub fn unassigned_count(&self) -> usize {
        self.pending.iter().filter(|e| e.activity.is_none()).count()
    }

    pub fn status(&self, now: NaiveDateTime) -> Status {
        let Some(open) = &self.open else {
            return Status::StampedOut;
        };
        let now = whole_seconds(now).max(open.start);
        let mut paused = open.paused_secs;
        if let Some(break_start) = open.break_started {
            paused += (now - break_start).num_seconds().max(0);
        }
        Status::StampedIn {
            since: open.start,
            activity: open.activity.clone(),
            on_break: open.break_started.is_some(),
            worked: Duration::seconds(((now - open.start).num_seconds() - paused).max(0)),
        }
    }

    fn close_open(&mut self, now: NaiveDateTime) -> usize {
        let Some(open) = self.open.take() else {
            return 0;
        };
        let entries = open.close(now);
        let count = entries.len();
        self.pending.extend(entries);
        count
    }

    pub fn default_path() -> anyhow::Result<PathBuf> {
        storage::data_file("stamps.json")
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        storage::read_json_or_default(path)
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        storage::write_json(path, self)
    }

    pub fn load() -> anyhow::Result<Self> {
        Self::load_from(&Self::default_path()?)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&Self::default_path()?)
    }
}

#[cfg(test)]
#[path = "stamp.tests.rs"]
mod tests;
