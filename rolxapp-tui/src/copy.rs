//! The local copy: the user's records and reference data, kept so the TUI works offline.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, Local, NaiveDate};
use rolx_core::api::activity::Activity;
use rolx_core::api::balance::Balance;
use rolx_core::api::records::Record;
use rolx_core::api::user_report::UserMonthReport;
use rolx_core::api::year_info::YearInfo;
use serde::{Deserialize, Serialize};

use crate::store::LocalStore;

/// How many months back and ahead the copy keeps.
const MONTHS_BACK: i32 = 24;
const MONTHS_AHEAD: i32 = 2;

/// A value together with when it was fetched.
#[derive(Clone, Serialize, Deserialize)]
pub struct Stamped<T> {
    pub at: DateTime<Local>,
    pub value: T,
}

/// What the server computes or lists, kept as last seen.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub activities: Vec<Activity>,
    pub favourites: Vec<Activity>,
    pub edit_lock: Option<NaiveDate>,
    pub balance: Option<Stamped<Balance>>,
    pub year_infos: BTreeMap<i32, Stamped<YearInfo>>,
    pub reports: BTreeMap<String, Stamped<UserMonthReport>>,
}

#[derive(Serialize, Deserialize)]
struct MonthFile {
    /// Set when the whole month was fetched, not just single days.
    complete_at: Option<DateTime<Local>>,
    records: Vec<Record>,
}

#[derive(Default)]
pub struct LocalCopy {
    days: BTreeMap<NaiveDate, Record>,
    months: BTreeMap<NaiveDate, DateTime<Local>>,
    pub reference: Reference,
}

pub fn record_date(record: &Record) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&record.date, "%Y-%m-%d").ok()
}

pub fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

pub fn month_file(month: NaiveDate) -> String {
    format!("records/{}.json", month.format("%Y-%m"))
}

/// The months the copy covers around `today`, oldest first.
pub fn window(today: NaiveDate) -> Vec<NaiveDate> {
    let first = first_of_month(today);
    (-MONTHS_BACK..=MONTHS_AHEAD)
        .filter_map(|offset| shift_months(first, offset))
        .collect()
}

fn shift_months(first: NaiveDate, offset: i32) -> Option<NaiveDate> {
    if offset >= 0 {
        first.checked_add_months(chrono::Months::new(offset as u32))
    } else {
        first.checked_sub_months(chrono::Months::new(offset.unsigned_abs()))
    }
}

impl LocalCopy {
    /// Reads everything stored; files that can't be read are reported and skipped.
    pub fn load(store: &LocalStore) -> (Self, Vec<String>) {
        let mut copy = Self::default();
        let mut problems = Vec::new();

        match store.read::<Reference>("reference.json") {
            Ok(reference) => copy.reference = reference.unwrap_or_default(),
            Err(error) => problems.push(error),
        }
        for name in store.list("records").unwrap_or_default() {
            match store.read::<MonthFile>(&format!("records/{name}")) {
                Ok(Some(file)) => {
                    let month = file
                        .records
                        .iter()
                        .find_map(record_date)
                        .map(first_of_month);
                    if let (Some(month), Some(at)) = (month, file.complete_at) {
                        copy.months.insert(month, at);
                    }
                    for record in file.records {
                        if let Some(date) = record_date(&record) {
                            copy.days.insert(date, record);
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => problems.push(error),
            }
        }
        (copy, problems)
    }

    pub fn day(&self, date: NaiveDate) -> Option<&Record> {
        self.days.get(&date)
    }

    /// The stored records from `from` up to, not including, `to`.
    pub fn days_between(&self, from: NaiveDate, to: NaiveDate) -> Vec<Record> {
        self.days.range(from..to).map(|(_, r)| r.clone()).collect()
    }

    /// When the whole month was last fetched, if it ever was.
    pub fn month_fetched(&self, month: NaiveDate) -> Option<DateTime<Local>> {
        self.months.get(&first_of_month(month)).copied()
    }

    /// Whether no whole month has been fetched yet, as on the very first start.
    pub fn is_empty(&self) -> bool {
        self.months.is_empty()
    }

    /// Forgets the months outside the window around `today`; returns them.
    pub fn prune(&mut self, today: NaiveDate) -> Vec<NaiveDate> {
        let kept = window(today);
        let outside: BTreeSet<NaiveDate> = self
            .days
            .keys()
            .map(|date| first_of_month(*date))
            .chain(self.months.keys().copied())
            .filter(|month| !kept.contains(month))
            .collect();
        self.days
            .retain(|date, _| !outside.contains(&first_of_month(*date)));
        self.months.retain(|month, _| !outside.contains(month));
        outside.into_iter().collect()
    }

    /// Stores single days; returns the months whose files need writing.
    pub fn put_days(&mut self, records: &[Record]) -> BTreeSet<NaiveDate> {
        let mut touched = BTreeSet::new();
        for record in records {
            if let Some(date) = record_date(record) {
                self.days.insert(date, record.clone());
                touched.insert(first_of_month(date));
            }
        }
        touched
    }

    /// Stores a whole month as fetched at `at`; days the server did not return are dropped.
    pub fn put_month(&mut self, month: NaiveDate, records: &[Record], at: DateTime<Local>) {
        let month = first_of_month(month);
        self.days.retain(|date, _| first_of_month(*date) != month);
        self.put_days(records);
        self.months.insert(month, at);
    }

    /// Writes the files of the given months.
    pub fn save_months(
        &self,
        store: &LocalStore,
        months: impl IntoIterator<Item = NaiveDate>,
    ) -> Result<(), String> {
        for month in months {
            let first = first_of_month(month);
            let records: Vec<Record> = self
                .days
                .iter()
                .filter(|(date, _)| first_of_month(**date) == first)
                .map(|(_, r)| r.clone())
                .collect();
            let file = MonthFile {
                complete_at: self.months.get(&first).copied(),
                records,
            };
            store.write(&month_file(first), &file)?;
        }
        Ok(())
    }

    pub fn save_reference(&self, store: &LocalStore) -> Result<(), String> {
        store.write("reference.json", &self.reference)
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::test_support::{main_app, temp_local_store};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn record(day: &str) -> Record {
        let mut record = main_app().record.unwrap();
        record.date = day.into();
        record
    }

    fn at() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 31, 12, 0, 0).unwrap()
    }

    #[test]
    fn the_window_covers_24_months_back_and_two_ahead() {
        let months = window(date(2026, 10, 31));

        assert_eq!(months.len(), 27);
        assert_eq!(months[0], date(2024, 10, 1));
        assert_eq!(months[24], date(2026, 10, 1));
        assert_eq!(*months.last().unwrap(), date(2026, 12, 1));
    }

    #[test]
    fn single_days_do_not_make_a_month_complete() {
        let mut copy = LocalCopy::default();

        copy.put_days(&[record("2026-10-30")]);

        assert!(copy.day(date(2026, 10, 30)).is_some());
        assert!(copy.month_fetched(date(2026, 10, 1)).is_none());
        assert!(copy.is_empty());
    }

    #[test]
    fn a_whole_month_replaces_its_days() {
        let mut copy = LocalCopy::default();
        copy.put_days(&[record("2026-10-29"), record("2026-09-30")]);

        copy.put_month(date(2026, 10, 1), &[record("2026-10-30")], at());

        assert!(copy.day(date(2026, 10, 29)).is_none(), "gone on the server");
        assert!(copy.day(date(2026, 10, 30)).is_some());
        assert!(copy.day(date(2026, 9, 30)).is_some(), "other months stay");
        assert_eq!(copy.month_fetched(date(2026, 10, 15)), Some(at()));
    }

    #[test]
    fn months_outside_the_window_are_pruned() {
        let mut copy = LocalCopy::default();
        copy.put_month(date(2024, 9, 1), &[record("2024-09-10")], at());
        copy.put_month(date(2024, 10, 1), &[record("2024-10-10")], at());
        copy.put_days(&[record("2027-03-01")]);

        let pruned = copy.prune(date(2026, 10, 31));

        assert_eq!(pruned, [date(2024, 9, 1), date(2027, 3, 1)]);
        assert!(copy.day(date(2024, 9, 10)).is_none());
        assert!(copy.month_fetched(date(2024, 9, 1)).is_none());
        assert!(copy.day(date(2024, 10, 10)).is_some());
    }

    #[test]
    fn days_between_is_half_open() {
        let mut copy = LocalCopy::default();
        copy.put_days(&[
            record("2026-10-26"),
            record("2026-10-27"),
            record("2026-11-02"),
        ]);

        let days = copy.days_between(date(2026, 10, 26), date(2026, 11, 2));

        assert_eq!(days.len(), 2);
    }

    #[test]
    fn saves_and_loads_months_and_reference() {
        let store = temp_local_store();
        let mut copy = LocalCopy::default();
        copy.put_month(date(2026, 10, 1), &[record("2026-10-30")], at());
        copy.put_days(&[record("2026-09-12")]);
        copy.reference.edit_lock = Some(date(2026, 10, 1));
        copy.save_months(&store, [date(2026, 10, 1), date(2026, 9, 1)])
            .unwrap();
        copy.save_reference(&store).unwrap();

        let (loaded, problems) = LocalCopy::load(&store);

        assert!(problems.is_empty(), "{problems:?}");
        assert!(loaded.day(date(2026, 10, 30)).is_some());
        assert!(loaded.day(date(2026, 9, 12)).is_some());
        assert_eq!(loaded.month_fetched(date(2026, 10, 1)), Some(at()));
        assert!(loaded.month_fetched(date(2026, 9, 1)).is_none());
        assert_eq!(loaded.reference.edit_lock, Some(date(2026, 10, 1)));
    }

    #[test]
    fn an_unreadable_file_is_reported_and_the_rest_still_loads() {
        let store = temp_local_store();
        let mut copy = LocalCopy::default();
        copy.put_month(date(2026, 10, 1), &[record("2026-10-30")], at());
        copy.save_months(&store, [date(2026, 10, 1)]).unwrap();
        std::fs::write(store.root().join("records/2026-09.json"), "garbage").unwrap();

        let (loaded, problems) = LocalCopy::load(&store);

        assert_eq!(problems.len(), 1);
        assert!(loaded.day(date(2026, 10, 30)).is_some());
    }
}
