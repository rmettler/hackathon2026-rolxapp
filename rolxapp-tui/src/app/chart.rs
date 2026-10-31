use std::collections::BTreeMap;

use super::App;
use super::absence::leave_time;
use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rolx_core::api::records::Record;

/// The server's `DayType.Workday`.
const WORKDAY: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChartMeasure {
    RunningTotal,
    DailyHours,
}

impl ChartMeasure {
    pub fn label(self) -> &'static str {
        match self {
            ChartMeasure::RunningTotal => "Running total",
            ChartMeasure::DailyHours => "Daily hours",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            ChartMeasure::RunningTotal => ChartMeasure::DailyHours,
            ChartMeasure::DailyHours => ChartMeasure::RunningTotal,
        }
    }
}

/// Booked time per day of the month, for one activity or for all of them.
pub struct ChartSeries {
    /// `None` for the combined series of all activities.
    pub activity_id: Option<i32>,
    pub number: String,
    pub daily: Vec<Duration>,
    pub total: Duration,
}

/// One month of booked time; `series[0]` is always the combined series.
pub struct MonthChart {
    pub days: u32,
    /// Last day with a data point: today in the current month, 0 for future months.
    pub last_day: u32,
    pub series: Vec<ChartSeries>,
    /// Target time of each day; zero on weekends and holidays.
    expected: Vec<Duration>,
    /// The line of the daily target: weekends and holidays keep the last working day's full target.
    level: Vec<Duration>,
}

fn days_in_month(first: NaiveDate) -> u32 {
    let (year, month) = if first.month() == 12 {
        (first.year() + 1, 1)
    } else {
        (first.year(), first.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).map_or(30, |next| (next - first).num_days() as u32)
}

impl MonthChart {
    pub fn build(month: NaiveDate, records: &[Record], today: NaiveDate) -> Self {
        let days = days_in_month(month);
        let blank = || vec![Duration::zero(); days as usize];
        let mut all = blank();
        let mut expected = blank();
        let mut full_target: Vec<Option<Duration>> = vec![None; days as usize];
        let mut by_activity: BTreeMap<i32, (String, Vec<Duration>)> = BTreeMap::new();

        for record in records {
            let Ok(date) = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d") else {
                continue;
            };
            if (date.year(), date.month()) != (month.year(), month.month()) {
                continue;
            }
            let day = date.day0() as usize;
            let nominal = Duration::seconds(record.nominal_work_time);
            if record.day_type == WORKDAY && nominal > Duration::zero() {
                full_target[day] = Some(nominal);
                expected[day] = nominal - leave_time(record).unwrap_or_else(Duration::zero);
            }
            for entry in &record.entries {
                all[day] += entry.duration;
                let activity = by_activity
                    .entry(entry.activity_id)
                    .or_insert_with(|| (entry.full_activity_number.clone(), blank()));
                activity.1[day] += entry.duration;
            }
        }

        let mut series: Vec<ChartSeries> = by_activity
            .into_iter()
            .map(|(id, (number, daily))| ChartSeries {
                activity_id: Some(id),
                number,
                total: daily.iter().copied().sum(),
                daily,
            })
            .filter(|s| s.total > Duration::zero())
            .collect();
        series.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.number.cmp(&b.number)));
        series.insert(
            0,
            ChartSeries {
                activity_id: None,
                number: String::new(),
                total: all.iter().copied().sum(),
                daily: all,
            },
        );

        let last_day = if (today.year(), today.month()) == (month.year(), month.month()) {
            today.day()
        } else if today < month {
            0
        } else {
            days
        };
        // A month starting on a weekend takes the first working day's target.
        let mut carried = full_target
            .iter()
            .flatten()
            .next()
            .copied()
            .unwrap_or_default();
        let level = (0..days as usize)
            .map(|day| match full_target[day] {
                Some(full) => {
                    carried = full;
                    expected[day]
                }
                None => carried,
            })
            .collect();

        Self {
            days,
            last_day,
            series,
            expected,
            level,
        }
    }

    pub fn has_time(&self) -> bool {
        self.series[0].total > Duration::zero()
    }

    pub fn has_target(&self) -> bool {
        self.expected.iter().any(|d| *d > Duration::zero())
    }

    /// `(day of month, hours)` of the target for every day of the month.
    pub fn target_points(&self, measure: ChartMeasure) -> Vec<(f64, f64)> {
        let hours = |d: Duration| d.num_seconds() as f64 / 3600.0;
        let mut running = 0.0;
        (1..=self.days)
            .map(|day| {
                let i = day as usize - 1;
                running += hours(self.expected[i]);
                let value = match measure {
                    ChartMeasure::RunningTotal => running,
                    ChartMeasure::DailyHours => hours(self.level[i]),
                };
                (f64::from(day), value)
            })
            .collect()
    }

    /// `(day of month, hours)` for each day up to `last_day`.
    pub fn points(&self, index: usize, measure: ChartMeasure) -> Vec<(f64, f64)> {
        let Some(series) = self.series.get(index) else {
            return Vec::new();
        };
        let mut running = 0.0;
        (1..=self.last_day)
            .map(|day| {
                let hours = series.daily[day as usize - 1].num_seconds() as f64 / 3600.0;
                running += hours;
                let value = match measure {
                    ChartMeasure::RunningTotal => running,
                    ChartMeasure::DailyHours => hours,
                };
                (f64::from(day), value)
            })
            .collect()
    }
}

impl App {
    pub fn month_chart(&self) -> Option<MonthChart> {
        let today = Local::now().date_naive();
        self.chart_records
            .as_ref()
            .map(|records| MonthChart::build(self.chart_month, records, today))
    }

    fn selected_chart_activity(&self) -> Option<i32> {
        self.month_chart()?.series.get(self.chart_sel)?.activity_id
    }

    /// Switches month, remembering the selected activity so it can be reselected once loaded.
    pub(crate) fn move_chart_month(&mut self, month: NaiveDate) {
        self.chart_keep = self.selected_chart_activity();
        self.chart_sel = 0;
        self.chart_month = month.with_day(1).unwrap_or(month);
        self.chart_records = None;
        self.load_charts();
    }

    pub(crate) fn on_charts_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.move_chart_month(self.chart_month - Duration::days(1));
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.move_chart_month(self.chart_month + Duration::days(32));
            }
            KeyCode::Char('t') => self.move_chart_month(Local::now().date_naive()),
            KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
            KeyCode::Char('m') => self.chart_measure = self.chart_measure.toggle(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use rolx_core::api::records::RecordEntry;

    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

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

    fn record(day: &str, entries: Vec<RecordEntry>) -> Record {
        Record {
            date: day.into(),
            user_id: "u".into(),
            day_type: 0,
            day_name: String::new(),
            nominal_work_time: 8 * 3600,
            paid_leave_type: None,
            paid_leave_reason: None,
            entries,
        }
    }

    fn october() -> Vec<Record> {
        vec![
            record(
                "2026-10-01",
                vec![entry(1, "P1-A", 60), entry(2, "P1-B", 180)],
            ),
            record(
                "2026-10-02",
                vec![entry(1, "P1-A", 30), entry(1, "P1-A", 30)],
            ),
        ]
    }

    #[test]
    fn counts_the_days_of_the_month() {
        assert_eq!(days_in_month(date(2026, 10, 1)), 31);
        assert_eq!(days_in_month(date(2026, 12, 1)), 31);
        assert_eq!(days_in_month(date(2026, 2, 1)), 28);
        assert_eq!(days_in_month(date(2028, 2, 1)), 29);
    }

    #[test]
    fn lists_all_first_then_activities_by_booked_time() {
        let chart = MonthChart::build(date(2026, 10, 1), &october(), date(2026, 11, 5));

        assert_eq!(chart.series.len(), 3);
        assert_eq!(chart.series[0].activity_id, None);
        assert_eq!(chart.series[0].total, Duration::hours(5));
        assert_eq!(chart.series[1].number, "P1-B", "3:00 booked beats 2:00");
        assert_eq!(chart.series[2].total, Duration::hours(2));
    }

    #[test]
    fn entries_of_one_activity_on_a_day_add_up() {
        let chart = MonthChart::build(date(2026, 10, 1), &october(), date(2026, 11, 5));

        assert_eq!(chart.series[2].daily[1], Duration::hours(1));
    }

    #[test]
    fn running_total_and_daily_hours_points() {
        let chart = MonthChart::build(date(2026, 10, 1), &october(), date(2026, 11, 5));

        let running = chart.points(0, ChartMeasure::RunningTotal);
        assert_eq!(running[..3], [(1.0, 4.0), (2.0, 5.0), (3.0, 5.0)]);

        let daily = chart.points(0, ChartMeasure::DailyHours);
        assert_eq!(daily[..3], [(1.0, 4.0), (2.0, 1.0), (3.0, 0.0)]);
    }

    #[test]
    fn the_line_stops_today_in_the_current_month() {
        let current = MonthChart::build(date(2026, 10, 1), &october(), date(2026, 10, 15));
        assert_eq!(current.points(0, ChartMeasure::RunningTotal).len(), 15);

        let past = MonthChart::build(date(2026, 10, 1), &october(), date(2026, 11, 20));
        assert_eq!(past.points(0, ChartMeasure::RunningTotal).len(), 31);

        let future = MonthChart::build(date(2026, 10, 1), &october(), date(2026, 9, 1));
        assert!(future.points(0, ChartMeasure::RunningTotal).is_empty());
    }

    #[test]
    fn other_months_and_empty_entries_are_ignored() {
        let records = vec![
            record("2026-09-30", vec![entry(1, "P1-A", 60)]),
            record("2026-10-03", vec![entry(2, "P1-B", 0)]),
        ];
        let chart = MonthChart::build(date(2026, 10, 1), &records, date(2026, 11, 5));

        assert!(!chart.has_time());
        assert_eq!(chart.series.len(), 1, "only the combined series is left");
    }

    /// A working day of 8.4 hours (a 42-hour week).
    fn workday(day: &str, entries: Vec<RecordEntry>) -> Record {
        Record {
            nominal_work_time: 30_240,
            ..record(day, entries)
        }
    }

    fn weekend(day: &str) -> Record {
        Record {
            day_type: 1,
            nominal_work_time: 0,
            ..record(day, Vec::new())
        }
    }

    fn close(points: &[(f64, f64)], expected: &[f64]) {
        let values: Vec<f64> = points.iter().map(|p| p.1).collect();
        assert_eq!(values.len(), expected.len(), "{values:?}");
        for (value, expected) in values.iter().zip(expected) {
            assert!(
                (value - expected).abs() < 1e-9,
                "{values:?} vs {expected:?}"
            );
        }
    }

    /// Thu 1 to Mon 5 October 2026 with a weekend in between.
    fn week_around_a_weekend() -> Vec<Record> {
        vec![
            workday("2026-10-01", vec![entry(1, "P1-A", 60)]),
            workday("2026-10-02", Vec::new()),
            weekend("2026-10-03"),
            weekend("2026-10-04"),
            workday("2026-10-05", Vec::new()),
        ]
    }

    #[test]
    fn the_running_target_adds_up_working_days_only() {
        let chart = MonthChart::build(
            date(2026, 10, 1),
            &week_around_a_weekend(),
            date(2026, 10, 5),
        );

        let target = chart.target_points(ChartMeasure::RunningTotal);

        close(&target[..5], &[8.4, 16.8, 16.8, 16.8, 25.2]);
    }

    #[test]
    fn the_daily_target_stays_flat_over_the_weekend() {
        let chart = MonthChart::build(
            date(2026, 10, 1),
            &week_around_a_weekend(),
            date(2026, 10, 5),
        );

        let target = chart.target_points(ChartMeasure::DailyHours);

        close(&target[..5], &[8.4; 5]);
    }

    #[test]
    fn a_month_starting_on_a_weekend_takes_the_first_working_days_target() {
        let records = vec![
            weekend("2026-11-01"),
            workday("2026-11-02", vec![entry(1, "P1-A", 60)]),
        ];
        let chart = MonthChart::build(date(2026, 11, 1), &records, date(2026, 11, 30));

        close(
            &chart.target_points(ChartMeasure::DailyHours)[..2],
            &[8.4, 8.4],
        );
        close(
            &chart.target_points(ChartMeasure::RunningTotal)[..2],
            &[0.0, 8.4],
        );
    }

    #[test]
    fn a_day_off_lowers_the_target_but_the_weekend_after_it_keeps_the_full_one() {
        let mut friday = workday("2026-10-02", Vec::new());
        friday.paid_leave_type = Some(0);
        let records = vec![
            workday("2026-10-01", vec![entry(1, "P1-A", 60)]),
            friday,
            weekend("2026-10-03"),
        ];
        let chart = MonthChart::build(date(2026, 10, 1), &records, date(2026, 10, 3));

        close(
            &chart.target_points(ChartMeasure::DailyHours)[..3],
            &[8.4, 0.0, 8.4],
        );
        close(
            &chart.target_points(ChartMeasure::RunningTotal)[..3],
            &[8.4, 8.4, 8.4],
        );
    }

    #[test]
    fn a_part_day_absence_leaves_the_booked_time_as_the_target() {
        let mut thursday = workday("2026-10-01", vec![entry(1, "P1-A", 240)]);
        thursday.paid_leave_type = Some(1);
        let chart = MonthChart::build(date(2026, 10, 1), &[thursday], date(2026, 10, 1));

        close(&chart.target_points(ChartMeasure::DailyHours)[..1], &[4.0]);
    }

    #[test]
    fn the_target_runs_to_the_end_of_the_month_while_booked_time_stops_today() {
        let chart = MonthChart::build(
            date(2026, 10, 1),
            &week_around_a_weekend(),
            date(2026, 10, 3),
        );

        assert_eq!(chart.points(0, ChartMeasure::RunningTotal).len(), 3);
        assert_eq!(chart.target_points(ChartMeasure::RunningTotal).len(), 31);
    }

    #[test]
    fn without_working_days_there_is_no_target() {
        let records = vec![weekend("2026-10-03")];
        let chart = MonthChart::build(date(2026, 10, 1), &records, date(2026, 11, 5));

        assert!(!chart.has_target());
    }

    #[test]
    fn the_measure_toggles() {
        assert_eq!(
            ChartMeasure::RunningTotal.toggle(),
            ChartMeasure::DailyHours
        );
        assert_eq!(
            ChartMeasure::DailyHours.toggle(),
            ChartMeasure::RunningTotal
        );
    }
}
