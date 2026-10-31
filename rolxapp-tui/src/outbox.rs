//! Pending changes: days saved locally that the server has not accepted yet.

use chrono::{DateTime, Local, NaiveDate};
use rolx_core::api::records::{Record, RecordEntry};
use serde::{Deserialize, Serialize};

use crate::edit::validate_entry;
use crate::store::LocalStore;

const FILE: &str = "outbox.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeState {
    /// Waiting to be uploaded.
    Pending,
    /// The day changed on the server too; the user decides.
    Conflict,
    /// The server refused it; shown with its message until fixed or discarded.
    Rejected,
}

impl ChangeState {
    /// One character for tight places such as the week grid.
    pub fn symbol(self) -> &'static str {
        match self {
            ChangeState::Pending => "↑",
            ChangeState::Conflict => "⚠",
            ChangeState::Rejected => "✗",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ChangeState::Pending => "pending",
            ChangeState::Conflict => "conflict",
            ChangeState::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingChange {
    pub date: NaiveDate,
    /// The day as it was before the first pending change.
    pub base: Record,
    /// The day as the user left it.
    pub record: Record,
    pub state: ChangeState,
    /// The server's message for a rejected change, or why syncing stopped.
    pub message: Option<String>,
    /// The server's day when it differed from `base`.
    pub server: Option<Record>,
    pub changed_at: DateTime<Local>,
}

#[derive(Default)]
pub struct Outbox {
    changes: Vec<PendingChange>,
}

impl Outbox {
    pub fn load(store: &LocalStore) -> Result<Self, String> {
        let changes = store.read::<Vec<PendingChange>>(FILE)?.unwrap_or_default();
        Ok(Self { changes })
    }

    pub fn save(&self, store: &LocalStore) -> Result<(), String> {
        store.write(FILE, &self.changes)
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn get(&self, date: NaiveDate) -> Option<&PendingChange> {
        self.changes.iter().find(|c| c.date == date)
    }

    pub fn get_mut(&mut self, date: NaiveDate) -> Option<&mut PendingChange> {
        self.changes.iter_mut().find(|c| c.date == date)
    }

    /// The changes by date, for listing.
    pub fn by_date(&self) -> Vec<&PendingChange> {
        let mut changes: Vec<&PendingChange> = self.changes.iter().collect();
        changes.sort_by_key(|c| c.date);
        changes
    }

    /// Changes in `state`.
    pub fn count(&self, state: ChangeState) -> usize {
        self.changes.iter().filter(|c| c.state == state).count()
    }

    /// Changes that need the user, not just a connection.
    pub fn needs_attention(&self) -> usize {
        self.len() - self.count(ChangeState::Pending)
    }

    /// The days waiting to be uploaded, oldest change first.
    pub fn upload_order(&self) -> Vec<NaiveDate> {
        let mut waiting: Vec<&PendingChange> = self
            .changes
            .iter()
            .filter(|c| c.state == ChangeState::Pending)
            .collect();
        waiting.sort_by_key(|c| c.changed_at);
        waiting.into_iter().map(|c| c.date).collect()
    }

    /// Records that the user left `date` as `record`. The first change of a day keeps `base`;
    /// later ones only replace `record`, and returning to the base removes the change.
    pub fn record_change(
        &mut self,
        base: &Record,
        record: Record,
        date: NaiveDate,
        at: DateTime<Local>,
    ) {
        let position = self.changes.iter().position(|c| c.date == date);
        let base = position.map_or_else(|| base.clone(), |i| self.changes[i].base.clone());
        if days_equal(&base, &record) {
            if let Some(i) = position {
                self.changes.remove(i);
            }
            return;
        }
        let change = PendingChange {
            date,
            base,
            record,
            state: ChangeState::Pending,
            message: None,
            server: None,
            changed_at: at,
        };
        match position {
            Some(i) => self.changes[i] = change,
            None => self.changes.push(change),
        }
    }

    pub fn remove(&mut self, date: NaiveDate) -> Option<PendingChange> {
        let i = self.changes.iter().position(|c| c.date == date)?;
        Some(self.changes.remove(i))
    }

    /// `record`, or the pending version of its day when there is one.
    pub fn overlay(&self, record: Record) -> Record {
        match crate::copy::record_date(&record).and_then(|d| self.get(d)) {
            Some(change) => change.record.clone(),
            None => record,
        }
    }
}

type EntryKey = (i32, Option<i32>, Option<i32>, i64, Option<String>);

fn entry_key(entry: &RecordEntry) -> EntryKey {
    let comment = entry
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    (
        entry.activity_id,
        entry.begin,
        entry.pause.filter(|p| *p != 0),
        entry.duration.num_seconds(),
        comment,
    )
}

/// Entries as the server stores them: zero-length ones are dropped on save.
fn kept(entries: &[RecordEntry]) -> impl Iterator<Item = &RecordEntry> {
    entries.iter().filter(|e| e.duration.num_seconds() > 0)
}

fn sorted_keys(entries: &[RecordEntry]) -> Vec<EntryKey> {
    let mut keys: Vec<EntryKey> = kept(entries).map(entry_key).collect();
    keys.sort();
    keys
}

/// For each entry of `record`, whether the change added it, that is `base` has no such entry.
pub fn changed_flags(base: &Record, record: &Record) -> Vec<bool> {
    let mut unmatched: Vec<EntryKey> = kept(&base.entries).map(entry_key).collect();
    record
        .entries
        .iter()
        .map(|entry| {
            if entry.duration.num_seconds() <= 0 {
                return false;
            }
            let key = entry_key(entry);
            match unmatched.iter().position(|k| *k == key) {
                Some(i) => {
                    unmatched.remove(i);
                    false
                }
                None => true,
            }
        })
        .collect()
}

/// Whether two versions of a day hold the same entries and absence.
pub fn days_equal(a: &Record, b: &Record) -> bool {
    let reason = |r: &Record| {
        r.paid_leave_type
            .and(r.paid_leave_reason.as_deref().map(str::trim))
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    a.paid_leave_type == b.paid_leave_type
        && reason(a) == reason(b)
        && sorted_keys(&a.entries) == sorted_keys(&b.entries)
}

/// Applies the changes from `base` to `mine` on top of `theirs`.
/// Fails when the two sides touched the same thing or the result is not a valid day.
pub fn merge_day(base: &Record, mine: &Record, theirs: &Record) -> Result<Record, String> {
    let mut entries: Vec<RecordEntry> = kept(&theirs.entries).cloned().collect();

    let mut mine_left: Vec<&RecordEntry> = kept(&mine.entries).collect();
    let mut removed = Vec::new();
    for entry in kept(&base.entries) {
        let key = entry_key(entry);
        match mine_left.iter().position(|m| entry_key(m) == key) {
            Some(i) => {
                mine_left.remove(i);
            }
            None => removed.push(entry),
        }
    }
    let added = mine_left;

    for entry in removed {
        let key = entry_key(entry);
        match entries.iter().position(|e| entry_key(e) == key) {
            Some(i) => {
                entries.remove(i);
            }
            None => {
                return Err(format!(
                    "{} was removed here but changed on the server",
                    entry.full_activity_number
                ));
            }
        }
    }
    let first_added = entries.len();
    entries.extend(added.into_iter().cloned());

    let leave_changed_here = mine.paid_leave_type != base.paid_leave_type
        || mine.paid_leave_reason != base.paid_leave_reason;
    let leave_changed_there = theirs.paid_leave_type != base.paid_leave_type
        || theirs.paid_leave_reason != base.paid_leave_reason;
    if leave_changed_here && leave_changed_there && !days_equal_leave(mine, theirs) {
        return Err("The absence of this day was changed here and on the server".into());
    }
    let (leave_type, leave_reason) = if leave_changed_here {
        (mine.paid_leave_type, mine.paid_leave_reason.clone())
    } else {
        (theirs.paid_leave_type, theirs.paid_leave_reason.clone())
    };

    for index in first_added..entries.len() {
        validate_entry(&entries, index)?;
    }

    let mut merged = theirs.clone();
    merged.entries = entries;
    merged.paid_leave_type = leave_type;
    merged.paid_leave_reason = leave_reason;
    Ok(merged)
}

fn days_equal_leave(a: &Record, b: &Record) -> bool {
    a.paid_leave_type == b.paid_leave_type && a.paid_leave_reason == b.paid_leave_reason
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};

    use super::*;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
    }

    fn at(minute: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 30, 12, minute, 0).unwrap()
    }

    fn entry(activity: i32, begin_hour: i32, minutes: i64) -> RecordEntry {
        RecordEntry {
            activity_id: activity,
            full_activity_number: format!("P-A{activity}"),
            duration: Duration::minutes(minutes),
            begin: Some(begin_hour * 3600),
            pause: None,
            comment: None,
        }
    }

    fn day(entries: Vec<RecordEntry>) -> Record {
        Record {
            date: "2026-10-30".into(),
            user_id: "u".into(),
            day_type: 0,
            day_name: "Friday".into(),
            nominal_work_time: 8 * 3600,
            paid_leave_type: None,
            paid_leave_reason: None,
            entries,
        }
    }

    #[test]
    fn days_are_equal_regardless_of_order_and_zero_length_entries() {
        let a = day(vec![entry(1, 8, 60), entry(2, 10, 30)]);
        let b = day(vec![entry(2, 10, 30), entry(1, 8, 60), entry(3, 12, 0)]);

        assert!(days_equal(&a, &b));
        assert!(!days_equal(&a, &day(vec![entry(1, 8, 60)])));
    }

    #[test]
    fn blank_comments_and_zero_pauses_do_not_count() {
        let mut other = entry(1, 8, 60);
        other.comment = Some("  ".into());
        other.pause = Some(0);

        assert!(days_equal(&day(vec![entry(1, 8, 60)]), &day(vec![other])));
    }

    #[test]
    fn the_first_change_keeps_the_base_and_later_ones_replace_the_record() {
        let mut outbox = Outbox::default();
        let base = day(vec![entry(1, 8, 60)]);

        outbox.record_change(&base, day(vec![entry(1, 8, 90)]), date(), at(1));
        let second = day(vec![entry(1, 8, 90), entry(2, 10, 30)]);
        outbox.record_change(&day(vec![entry(1, 8, 90)]), second.clone(), date(), at(2));

        let change = outbox.get(date()).unwrap();
        assert!(days_equal(&change.base, &base));
        assert!(days_equal(&change.record, &second));
        assert_eq!(outbox.len(), 1);
    }

    #[test]
    fn returning_to_the_base_removes_the_change() {
        let mut outbox = Outbox::default();
        let base = day(vec![entry(1, 8, 60)]);
        outbox.record_change(&base, day(vec![]), date(), at(1));

        outbox.record_change(&day(vec![]), base.clone(), date(), at(2));

        assert!(outbox.is_empty());
    }

    #[test]
    fn saving_a_rejected_day_again_makes_it_pending() {
        let mut outbox = Outbox::default();
        let base = day(vec![]);
        outbox.record_change(&base, day(vec![entry(1, 8, 60)]), date(), at(1));
        let change = outbox.get_mut(date()).unwrap();
        change.state = ChangeState::Rejected;
        change.message = Some("Activity is closed".into());

        outbox.record_change(&base, day(vec![entry(1, 8, 45)]), date(), at(2));

        let change = outbox.get(date()).unwrap();
        assert_eq!(change.state, ChangeState::Pending);
        assert_eq!(change.message, None);
    }

    #[test]
    fn uploads_oldest_change_first_and_skips_the_ones_needing_attention() {
        let mut outbox = Outbox::default();
        let mut later = day(vec![]);
        later.date = "2026-10-31".into();
        let later_date = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        outbox.record_change(&later, day_with(&later, entry(1, 8, 60)), later_date, at(1));
        outbox.record_change(&day(vec![]), day(vec![entry(1, 8, 60)]), date(), at(2));
        assert_eq!(outbox.upload_order(), [later_date, date()]);

        outbox.get_mut(later_date).unwrap().state = ChangeState::Conflict;

        assert_eq!(outbox.upload_order(), [date()]);
        assert_eq!(outbox.needs_attention(), 1);
    }

    fn day_with(base: &Record, entry: RecordEntry) -> Record {
        let mut record = base.clone();
        record.entries.push(entry);
        record
    }

    #[test]
    fn overlay_shows_the_pending_day() {
        let mut outbox = Outbox::default();
        let mine = day(vec![entry(1, 8, 60)]);
        outbox.record_change(&day(vec![]), mine.clone(), date(), at(1));

        assert!(days_equal(&outbox.overlay(day(vec![])), &mine));
        let mut other_day = day(vec![]);
        other_day.date = "2026-10-29".into();
        assert!(outbox.overlay(other_day).entries.is_empty());
    }

    #[test]
    fn saves_and_loads() {
        let store = crate::test_support::temp_local_store();
        let mut outbox = Outbox::default();
        outbox.record_change(&day(vec![]), day(vec![entry(1, 8, 60)]), date(), at(1));
        outbox.save(&store).unwrap();

        let loaded = Outbox::load(&store).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get(date()).unwrap().state, ChangeState::Pending);
    }

    #[test]
    fn only_entries_the_change_added_are_flagged() {
        let base = day(vec![entry(1, 8, 60)]);
        let record = day(vec![entry(1, 8, 60), entry(2, 10, 30), entry(1, 8, 90)]);

        assert_eq!(changed_flags(&base, &record), [false, true, true]);
    }

    #[test]
    fn merge_adds_my_entries_to_theirs() {
        let base = day(vec![entry(1, 8, 60)]);
        let mine = day(vec![entry(1, 8, 60), entry(2, 14, 60)]);
        let theirs = day(vec![entry(1, 8, 60), entry(3, 10, 60)]);

        let merged = merge_day(&base, &mine, &theirs).unwrap();

        assert!(days_equal(
            &merged,
            &day(vec![entry(1, 8, 60), entry(3, 10, 60), entry(2, 14, 60)])
        ));
    }

    #[test]
    fn merge_removes_what_i_removed() {
        let base = day(vec![entry(1, 8, 60), entry(2, 10, 60)]);
        let mine = day(vec![entry(1, 8, 60)]);
        let theirs = day(vec![entry(1, 8, 60), entry(2, 10, 60), entry(3, 12, 60)]);

        let merged = merge_day(&base, &mine, &theirs).unwrap();

        assert!(days_equal(
            &merged,
            &day(vec![entry(1, 8, 60), entry(3, 12, 60)])
        ));
    }

    #[test]
    fn merge_fails_when_an_entry_i_removed_was_changed_there() {
        let base = day(vec![entry(1, 8, 60)]);
        let mine = day(vec![]);
        let theirs = day(vec![entry(1, 8, 90)]);

        assert!(
            merge_day(&base, &mine, &theirs)
                .unwrap_err()
                .contains("removed here")
        );
    }

    #[test]
    fn merge_fails_on_overlap() {
        let base = day(vec![]);
        let mine = day(vec![entry(2, 9, 60)]);
        let theirs = day(vec![entry(1, 8, 120)]);

        assert!(
            merge_day(&base, &mine, &theirs)
                .unwrap_err()
                .contains("Overlaps")
        );
    }

    #[test]
    fn merge_takes_my_absence_unless_both_changed_it() {
        let base = day(vec![]);
        let mut mine = base.clone();
        mine.paid_leave_type = Some(0);
        let theirs = base.clone();

        assert_eq!(
            merge_day(&base, &mine, &theirs).unwrap().paid_leave_type,
            Some(0)
        );

        let mut theirs = base.clone();
        theirs.paid_leave_type = Some(1);
        assert!(merge_day(&base, &mine, &theirs).is_err());
    }
}
