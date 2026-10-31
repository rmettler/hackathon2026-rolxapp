use std::collections::BTreeMap;

use super::App;
use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rolx_core::api::records::Record;

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
        let mut by_activity: BTreeMap<i32, (String, Vec<Duration>)> = BTreeMap::new();

        for record in records {
            let Ok(date) = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d") else {
                continue;
            };
            if (date.year(), date.month()) != (month.year(), month.month()) {
                continue;
            }
            let day = date.day0() as usize;
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
        Self {
            days,
            last_day,
            series,
        }
    }

    pub fn has_time(&self) -> bool {
        self.series[0].total > Duration::zero()
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
