use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};
use rolx_core::api::activity::Activity;
use rolx_core::api::fmt_duration;
use rolx_core::api::records::Record;

use super::absence::{Absence, absence_of};
use super::activities::is_bookable;

/// Time booked on one activity on one day.
#[derive(Clone, Copy)]
pub struct Cell {
    pub duration: Duration,
    pub entries: usize,
}

impl Cell {
    fn empty() -> Self {
        Self {
            duration: Duration::zero(),
            entries: 0,
        }
    }

    /// `h:mm`, with a `+` when several entries add up to it; blank when nothing is booked.
    pub fn text(&self) -> String {
        match self.entries {
            0 => String::new(),
            1 => fmt_duration(self.duration),
            _ => format!("{}+", fmt_duration(self.duration)),
        }
    }
}

pub struct GridRow {
    pub activity_id: i32,
    pub number: String,
    pub favourite: bool,
    pub cells: [Cell; 7],
}

impl GridRow {
    fn new(activity_id: i32, number: String, favourite: bool) -> Self {
        Self {
            activity_id,
            number,
            favourite,
            cells: [Cell::empty(); 7],
        }
    }

    pub fn total(&self) -> Duration {
        self.cells.iter().map(|c| c.duration).sum()
    }
}

/// A week of records as activities (rows) by weekday (columns, Monday first).
pub struct WeekGrid {
    pub rows: Vec<GridRow>,
    pub totals: [Duration; 7],
    pub targets: [Duration; 7],
    /// `DayType` of each column; anything but 0 is a weekend or holiday.
    pub day_types: [i32; 7],
    /// Absence booked on each column's day.
    pub absences: [Option<Absence>; 7],
}

impl WeekGrid {
    /// Every favourite that is bookable this week gets a row, so its cells can be booked.
    pub fn build(monday: NaiveDate, records: &[Record], favourites: &[Activity]) -> Self {
        let mut rows: BTreeMap<i32, GridRow> = BTreeMap::new();
        let bookable_this_week =
            |a: &Activity| (0..7).any(|d| is_bookable(a, monday + Duration::days(d)));
        for favourite in favourites.iter().filter(|a| bookable_this_week(a)) {
            rows.entry(favourite.id)
                .or_insert_with(|| GridRow::new(favourite.id, favourite.full_number.clone(), true));
        }
        let mut totals = [Duration::zero(); 7];
        let mut targets = [Duration::zero(); 7];
        let mut day_types = [0; 7];
        let mut absences = [None; 7];

        for record in records {
            let Ok(date) = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d") else {
                continue;
            };
            let Ok(col) = usize::try_from((date - monday).num_days()) else {
                continue;
            };
            if col >= 7 {
                continue;
            }
            targets[col] = Duration::seconds(record.nominal_work_time);
            day_types[col] = record.day_type;
            absences[col] = absence_of(record);
            for entry in &record.entries {
                let row = rows.entry(entry.activity_id).or_insert_with(|| {
                    GridRow::new(entry.activity_id, entry.full_activity_number.clone(), false)
                });
                let cell = &mut row.cells[col];
                cell.duration += entry.duration;
                cell.entries += 1;
                totals[col] += entry.duration;
            }
        }

        let mut rows: Vec<GridRow> = rows.into_values().collect();
        rows.sort_by(|a, b| {
            b.favourite
                .cmp(&a.favourite)
                .then_with(|| a.number.cmp(&b.number))
        });
        Self {
            rows,
            totals,
            targets,
            day_types,
            absences,
        }
    }

    pub fn week_total(&self) -> Duration {
        self.totals.iter().copied().sum()
    }

    pub fn week_target(&self) -> Duration {
        self.targets.iter().copied().sum()
    }
}

#[cfg(test)]
mod tests {
    use rolx_core::api::records::RecordEntry;

    use super::*;

    fn entry(activity_id: i32, number: &str, minutes: i64) -> RecordEntry {
        RecordEntry {
            activity_id,
            full_activity_number: number.into(),
            duration: Duration::minutes(minutes),
            begin: None,
            pause: None,
            comment: None,
        }
    }

    fn record(date: &str, day_type: i32, entries: Vec<RecordEntry>) -> Record {
        Record {
            date: date.into(),
            user_id: "u".into(),
            day_type,
            day_name: String::new(),
            nominal_work_time: 8 * 3600,
            paid_leave_type: None,
            paid_leave_reason: None,
            entries,
        }
    }

    fn monday() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()
    }

    #[test]
    fn groups_entries_by_activity_and_weekday() {
        let records = [
            record(
                "2026-10-26",
                0,
                vec![entry(2, "P1-B", 60), entry(1, "P1-A", 30)],
            ),
            record("2026-10-28", 0, vec![entry(1, "P1-A", 120)]),
            record("2026-10-31", 1, vec![]),
        ];
        let grid = WeekGrid::build(monday(), &records, &[]);

        assert_eq!(grid.rows.len(), 2);
        assert_eq!(grid.rows[0].number, "P1-A");
        assert_eq!(grid.rows[0].cells[0].duration, Duration::minutes(30));
        assert_eq!(grid.rows[0].cells[2].duration, Duration::minutes(120));
        assert_eq!(grid.rows[0].total(), Duration::minutes(150));
        assert_eq!(grid.rows[1].cells[0].entries, 1);
        assert_eq!(grid.rows[1].cells[1].entries, 0);
        assert_eq!(grid.totals[0], Duration::minutes(90));
        assert_eq!(grid.week_total(), Duration::minutes(210));
        assert_eq!(grid.day_types[5], 1);
        assert_eq!(grid.targets[0], Duration::hours(8));
    }

    #[test]
    fn sums_several_entries_of_one_activity_in_a_cell() {
        let records = [record(
            "2026-10-27",
            0,
            vec![entry(1, "P1-A", 60), entry(1, "P1-A", 45)],
        )];
        let grid = WeekGrid::build(monday(), &records, &[]);

        assert_eq!(grid.rows.len(), 1);
        assert_eq!(grid.rows[0].cells[1].duration, Duration::minutes(105));
        assert_eq!(grid.rows[0].cells[1].entries, 2);
    }

    #[test]
    fn ignores_records_outside_the_week() {
        let records = [
            record("2026-10-25", 0, vec![entry(1, "P1-A", 60)]),
            record("2026-11-02", 0, vec![entry(1, "P1-A", 60)]),
        ];
        let grid = WeekGrid::build(monday(), &records, &[]);

        assert!(grid.rows.is_empty());
        assert_eq!(grid.week_total(), Duration::zero());
    }

    fn favourite(id: i32, number: &str) -> Activity {
        Activity {
            id,
            full_number: number.into(),
            ..crate::test_support::main_app().suitable[0].clone()
        }
    }

    #[test]
    fn a_favourite_without_entries_still_gets_a_row() {
        let grid = WeekGrid::build(monday(), &[], &[favourite(7, "P9-F")]);

        assert_eq!(grid.rows.len(), 1);
        assert!(grid.rows[0].favourite);
        assert_eq!(grid.rows[0].number, "P9-F");
        assert!(grid.rows[0].cells.iter().all(|c| c.entries == 0));
    }

    #[test]
    fn favourites_merge_with_their_entries_and_sort_first() {
        let records = [record(
            "2026-10-27",
            0,
            vec![entry(1, "P1-A", 30), entry(7, "P9-F", 60)],
        )];
        let grid = WeekGrid::build(monday(), &records, &[favourite(7, "P9-F")]);

        assert_eq!(grid.rows.len(), 2);
        assert_eq!(grid.rows[0].activity_id, 7, "favourite sorts before P1-A");
        assert_eq!(grid.rows[0].cells[1].duration, Duration::minutes(60));
        assert!(!grid.rows[1].favourite);
    }
}
