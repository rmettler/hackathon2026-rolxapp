use std::collections::BTreeMap;

use super::App;
use super::absence::leave_time;
use chrono::{Datelike, Duration, Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rolx_core::api::records::Record;

/// The server's `DayType.Workday`.
const WORKDAY: i32 = 0;

/// Which chart the right panel of the Charts tab shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChartView {
    Line,
    Pie,
}

/// Activities beyond this many biggest ones are combined into one slice.
const MAX_SLICES: usize = 8;

/// How much of the radius a slice keeps while another one is highlighted.
const DIMMED_RADIUS: f64 = 0.88;

/// One slice of the pie: an activity's share of the month's booked time.
#[derive(Clone)]
pub struct Slice {
    /// `None` for the combined "Other" slice, and for the comment slices of a detail pie chart.
    pub activity_id: Option<i32>,
    /// The activity number, or the comment in a detail pie chart; "Other" for the combined slice.
    pub number: String,
    pub total: Duration,
    pub share: f64,
}

/// The comment of entries that have none.
const NO_COMMENT: &str = "(no comment)";

/// The activity whose comments the detail pie chart shows.
#[derive(Clone, Debug)]
pub struct ChartDetail {
    pub activity_id: i32,
    /// The row of the activity list to select again when the detail is closed.
    pub back_to: usize,
}

/// The booked time of one activity in a month, split by comment.
pub struct DetailChart {
    pub total: Duration,
    /// Every comment, biggest first.
    pub comments: Vec<Slice>,
}

impl DetailChart {
    pub fn slices(&self) -> Vec<Slice> {
        fold(self.comments.clone())
    }
}

/// Keeps the biggest slices and combines the rest into "Other".
fn fold(mut slices: Vec<Slice>) -> Vec<Slice> {
    if slices.len() > MAX_SLICES {
        let rest = slices.split_off(MAX_SLICES);
        slices.push(Slice {
            activity_id: None,
            number: "Other".into(),
            total: rest.iter().map(|s| s.total).sum(),
            share: rest.iter().map(|s| s.share).sum(),
        });
    }
    slices
}

/// The slice a row of a pie's list stands for; the first row, the total, has none.
pub fn slice_of_row(row: usize) -> Option<usize> {
    (row > 0).then(|| (row - 1).min(MAX_SLICES))
}

/// The slice at `(x, y)` of a unit pie starting at 12 o'clock, going clockwise.
/// While `selected` is set, the other slices are drawn smaller.
pub fn slice_at(slices: &[Slice], x: f64, y: f64, selected: Option<usize>) -> Option<usize> {
    let angle = x.atan2(y).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
    let mut end = 0.0;
    for (i, slice) in slices.iter().enumerate() {
        end += slice.share;
        // The last slice also takes what rounding leaves over.
        if angle < end || i + 1 == slices.len() {
            let radius = if selected.is_some_and(|s| s != i) {
                DIMMED_RADIUS
            } else {
                1.0
            };
            return (x.hypot(y) <= radius).then_some(i);
        }
    }
    None
}

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
    /// Booked time of each activity by comment.
    comments: BTreeMap<i32, BTreeMap<String, Duration>>,
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
        let mut comments: BTreeMap<i32, BTreeMap<String, Duration>> = BTreeMap::new();

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
                let comment = entry
                    .comment
                    .as_deref()
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .unwrap_or(NO_COMMENT);
                *comments
                    .entry(entry.activity_id)
                    .or_default()
                    .entry(comment.to_string())
                    .or_insert_with(Duration::zero) += entry.duration;
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
            comments,
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

    /// The biggest activities by booked time, the rest combined as "Other" when there are many.
    pub fn slices(&self) -> Vec<Slice> {
        let total = self.series[0].total;
        if total <= Duration::zero() {
            return Vec::new();
        }
        let share = |d: Duration| d.num_seconds() as f64 / total.num_seconds() as f64;
        let slices = self.series[1..]
            .iter()
            .map(|s| Slice {
                activity_id: s.activity_id,
                number: s.number.clone(),
                total: s.total,
                share: share(s.total),
            })
            .collect();
        fold(slices)
    }

    /// The slice an entry of the activity list stands for; "All activities" has none.
    pub fn slice_of_row(&self, row: usize) -> Option<usize> {
        slice_of_row(row)
    }

    /// One activity's booked time split by comment, `None` if it has none this month.
    pub fn detail(&self, activity_id: i32) -> Option<DetailChart> {
        let total = self
            .series
            .iter()
            .find(|s| s.activity_id == Some(activity_id))?
            .total;
        let mut comments: Vec<Slice> = self
            .comments
            .get(&activity_id)?
            .iter()
            .filter(|(_, d)| **d > Duration::zero())
            .map(|(comment, d)| Slice {
                activity_id: None,
                number: comment.clone(),
                total: *d,
                share: d.num_seconds() as f64 / total.num_seconds() as f64,
            })
            .collect();
        comments.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.number.cmp(&b.number)));
        Some(DetailChart { total, comments })
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

    /// Rows of the activity list, or of the comment list while the detail pie chart is open.
    pub(crate) fn chart_list_len(&self) -> usize {
        let Some(chart) = self.month_chart() else {
            return 0;
        };
        match self
            .chart_detail
            .as_ref()
            .and_then(|d| chart.detail(d.activity_id))
        {
            Some(detail) => detail.comments.len() + 1,
            None => chart.series.len(),
        }
    }

    /// Keeps the selection sensible after the month's records arrived.
    pub(crate) fn settle_chart_selection(&mut self) {
        let keep = self.chart_keep.take();
        let Some(chart) = self.month_chart() else {
            return;
        };
        if let Some(detail) = self.chart_detail.clone() {
            let position = chart
                .series
                .iter()
                .position(|s| s.activity_id == Some(detail.activity_id));
            match (position, chart.detail(detail.activity_id)) {
                (Some(row), Some(shown)) => {
                    self.chart_detail = Some(ChartDetail {
                        back_to: row,
                        ..detail
                    });
                    self.chart_sel = self.chart_sel.min(shown.comments.len());
                }
                _ => {
                    // The activity has no booked time in this month.
                    self.chart_detail = None;
                    self.chart_sel = 0;
                }
            }
            return;
        }
        self.chart_sel = match keep {
            Some(id) => chart
                .series
                .iter()
                .position(|s| s.activity_id == Some(id))
                .unwrap_or(0),
            None => self.chart_sel.min(chart.series.len().saturating_sub(1)),
        };
    }

    /// `Enter`: opens the detail pie chart of the selected activity, or closes it again.
    fn toggle_chart_detail(&mut self) {
        if self.close_chart_detail() {
            return;
        }
        let Some(chart) = self.month_chart() else {
            return;
        };
        match chart.series.get(self.chart_sel).and_then(|s| s.activity_id) {
            Some(activity_id) => {
                self.chart_detail = Some(ChartDetail {
                    activity_id,
                    back_to: self.chart_sel,
                });
                self.chart_sel = 0;
            }
            None => self.status = Some("Select an activity first".into()),
        }
    }

    /// Returns to the overview pie chart with the activity selected again; false if it wasn't open.
    fn close_chart_detail(&mut self) -> bool {
        let Some(detail) = self.chart_detail.take() else {
            return false;
        };
        self.chart_sel = detail.back_to;
        true
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
            KeyCode::Char('m') if self.chart_view == ChartView::Line => {
                self.chart_measure = self.chart_measure.toggle();
            }
            KeyCode::Enter if self.chart_view == ChartView::Pie => self.toggle_chart_detail(),
            KeyCode::Esc => {
                self.close_chart_detail();
            }
            KeyCode::Char('p') => {
                self.close_chart_detail();
                self.chart_view = match self.chart_view {
                    ChartView::Line => ChartView::Pie,
                    ChartView::Pie => ChartView::Line,
                };
            }
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

    /// A month where activity `n` has `n` hours booked on the 1st, for `count` activities.
    fn many_activities(count: i32) -> MonthChart {
        let entries = (1..=count)
            .map(|n| entry(n, &format!("P{n}"), i64::from(n) * 60))
            .collect();
        MonthChart::build(
            date(2026, 10, 1),
            &[record("2026-10-01", entries)],
            date(2026, 11, 5),
        )
    }

    #[test]
    fn the_pie_has_a_slice_per_activity_biggest_first() {
        let slices = many_activities(3).slices();

        let numbers: Vec<&str> = slices.iter().map(|s| s.number.as_str()).collect();
        assert_eq!(numbers, ["P3", "P2", "P1"]);
        assert!((slices[0].share - 0.5).abs() < 1e-9, "3 of 6 hours");
    }

    #[test]
    fn activities_beyond_the_eighth_are_combined_into_other() {
        let slices = many_activities(10).slices();

        assert_eq!(slices.len(), 9);
        let other = slices.last().unwrap();
        assert_eq!(other.number, "Other");
        assert_eq!(other.total, Duration::hours(3), "P2 and P1");
        let sum: f64 = slices.iter().map(|s| s.share).sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn eight_activities_need_no_other_slice() {
        let slices = many_activities(8).slices();

        assert_eq!(slices.len(), 8);
        assert!(slices.iter().all(|s| s.number != "Other"));
    }

    #[test]
    fn a_month_without_booked_time_has_no_slices() {
        let chart = MonthChart::build(date(2026, 10, 1), &[], date(2026, 11, 5));

        assert!(chart.slices().is_empty());
    }

    #[test]
    fn list_rows_map_to_slices_and_all_activities_has_none() {
        let chart = many_activities(10);

        assert_eq!(chart.slice_of_row(0), None);
        assert_eq!(chart.slice_of_row(1), Some(0));
        assert_eq!(chart.slice_of_row(8), Some(7));
        assert_eq!(chart.slice_of_row(9), Some(8), "the first folded activity");
        assert_eq!(chart.slice_of_row(10), Some(8), "and the rest");
    }

    fn halves() -> Vec<Slice> {
        let slice = |number: &str| Slice {
            activity_id: Some(1),
            number: number.into(),
            total: Duration::hours(1),
            share: 0.5,
        };
        vec![slice("A"), slice("B")]
    }

    #[test]
    fn the_pie_starts_at_twelve_and_goes_clockwise() {
        let slices = halves();

        assert_eq!(slice_at(&slices, 0.0, 0.5, None), Some(0), "12 o'clock");
        assert_eq!(slice_at(&slices, 0.5, 0.0, None), Some(0), "3 o'clock");
        assert_eq!(slice_at(&slices, 0.0, -0.5, None), Some(1), "6 o'clock");
        assert_eq!(slice_at(&slices, -0.5, 0.0, None), Some(1), "9 o'clock");
    }

    #[test]
    fn nothing_is_drawn_outside_the_circle() {
        assert_eq!(slice_at(&halves(), 0.8, 0.8, None), None);
    }

    #[test]
    fn the_other_slices_shrink_while_one_is_highlighted() {
        let slices = halves();

        assert_eq!(slice_at(&slices, 0.0, 0.95, Some(0)), Some(0));
        assert_eq!(slice_at(&slices, 0.0, -0.95, Some(0)), None);
        assert_eq!(slice_at(&slices, 0.0, -0.95, None), Some(1));
    }

    fn commented(
        activity_id: i32,
        number: &str,
        minutes: i64,
        comment: Option<&str>,
    ) -> RecordEntry {
        RecordEntry {
            comment: comment.map(str::to_string),
            ..entry(activity_id, number, minutes)
        }
    }

    fn detail_of(entries: Vec<RecordEntry>) -> Option<DetailChart> {
        MonthChart::build(
            date(2026, 10, 1),
            &[record("2026-10-01", entries)],
            date(2026, 11, 5),
        )
        .detail(1)
    }

    #[test]
    fn the_detail_splits_one_activity_by_comment_biggest_first() {
        let detail = detail_of(vec![
            commented(1, "P1", 30, Some("plan")),
            commented(1, "P1", 90, Some("review")),
            commented(2, "P2", 60, Some("elsewhere")),
        ])
        .unwrap();

        let comments: Vec<&str> = detail.comments.iter().map(|c| c.number.as_str()).collect();
        assert_eq!(comments, ["review", "plan"]);
        assert_eq!(detail.total, Duration::hours(2));
        assert!((detail.comments[0].share - 0.75).abs() < 1e-9);
    }

    #[test]
    fn the_same_comment_adds_up_even_with_stray_spaces() {
        let detail = detail_of(vec![
            commented(1, "P1", 30, Some("review")),
            commented(1, "P1", 30, Some("  review ")),
        ])
        .unwrap();

        assert_eq!(detail.comments.len(), 1);
        assert_eq!(detail.comments[0].total, Duration::hours(1));
    }

    #[test]
    fn comments_differing_in_case_are_different_slices() {
        let detail = detail_of(vec![
            commented(1, "P1", 30, Some("review")),
            commented(1, "P1", 30, Some("Review")),
        ])
        .unwrap();

        assert_eq!(detail.comments.len(), 2);
    }

    #[test]
    fn entries_without_a_comment_share_one_slice() {
        let detail = detail_of(vec![
            commented(1, "P1", 30, None),
            commented(1, "P1", 15, Some("   ")),
        ])
        .unwrap();

        assert_eq!(detail.comments.len(), 1);
        assert_eq!(detail.comments[0].number, "(no comment)");
        assert_eq!(detail.comments[0].total, Duration::minutes(45));
    }

    #[test]
    fn comments_without_booked_time_get_no_slice() {
        let detail = detail_of(vec![
            commented(1, "P1", 30, Some("plan")),
            commented(1, "P1", 0, Some("nothing")),
        ])
        .unwrap();

        assert_eq!(detail.comments.len(), 1);
    }

    #[test]
    fn an_activity_without_booked_time_has_no_detail() {
        assert!(detail_of(vec![commented(2, "P2", 30, None)]).is_none());
    }

    #[test]
    fn only_the_eight_biggest_comments_get_a_slice_and_the_rest_is_other() {
        let entries = (1..=10)
            .map(|n| commented(1, "P1", i64::from(n) * 10, Some(&format!("c{n}"))))
            .collect();
        let detail = detail_of(entries).unwrap();

        assert_eq!(detail.comments.len(), 10, "the list keeps all of them");
        let slices = detail.slices();
        assert_eq!(slices.len(), 9);
        let other = slices.last().unwrap();
        assert_eq!(other.number, "Other");
        assert_eq!(other.total, Duration::minutes(30), "c1 and c2");
        let sum: f64 = slices.iter().map(|s| s.share).sum();
        assert!((sum - 1.0).abs() < 1e-9);
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
