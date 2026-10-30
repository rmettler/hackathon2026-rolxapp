//! The working week: what is booked, what the target is, and what is left.
//!
//! The target is the sum of the server's daily nominal times, which already are 0 on weekends and
//! holidays and include the user's part-time factor (pensum).

use chrono::{Datelike, Duration, NaiveDate};
use serde_json::Value;

use crate::client::RolxClient;
use crate::dayplan::{DayEntry, entries_of};
use crate::stamp::PendingEntry;

/// Target for a single day, independent of pensum: 8.4 hours.
pub const DAILY_TARGET_SECS: i64 = 30_240;

#[derive(Debug, Clone, PartialEq)]
pub struct WeekDay {
    pub date: NaiveDate,
    /// What the server expects for this day (pensum and holidays applied).
    pub nominal_secs: i64,
    /// Paid leave (vacation, sickness, ...) counts as worked.
    pub on_leave: bool,
    pub entries: Vec<DayEntry>,
}

/// Monday to Sunday.
#[derive(Debug, Clone, PartialEq)]
pub struct Week {
    pub days: Vec<WeekDay>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityTotal {
    pub activity_id: i32,
    pub number: String,
    pub secs: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DaySummary {
    pub date: NaiveDate,
    pub nominal_secs: i64,
    pub on_leave: bool,
    pub booked_secs: i64,
    /// Largest first.
    pub by_activity: Vec<ActivityTotal>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeekSummary {
    pub days: Vec<DaySummary>,
    pub target_secs: i64,
    pub booked_secs: i64,
    /// Nominal time of paid-leave days, credited as worked.
    pub leave_secs: i64,
    /// Negative when the target is exceeded.
    pub remaining_secs: i64,
    /// Part-time factor in percent, derived from the server's nominal times.
    pub pensum_percent: Option<u32>,
    /// Over the whole week, largest first.
    pub activities: Vec<ActivityTotal>,
}

/// The Monday of the week `day` is in.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - Duration::days(i64::from(day.weekday().num_days_from_monday()))
}

/// Builds a week from the day records the server returns for a range.
pub fn parse_week(records: &[Value]) -> Result<Week, serde_json::Error> {
    let mut days = Vec::new();
    for record in records {
        let date: NaiveDate = serde_json::from_value(record["date"].clone())?;
        days.push(WeekDay {
            date,
            nominal_secs: record["nominalWorkTime"].as_i64().unwrap_or(0),
            on_leave: !record["paidLeaveType"].is_null(),
            entries: entries_of(record)?,
        });
    }
    days.sort_by_key(|d| d.date);
    Ok(Week { days })
}

pub async fn fetch_week(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Week> {
    let start = week_start(day);
    let path = format!(
        "api/v1/WorkRecord/{}/range/{}..{}",
        client.session.user_id,
        start.format("%Y-%m-%d"),
        (start + Duration::days(7)).format("%Y-%m-%d"),
    );
    let records: Vec<Value> = client.get_json(&path).await?;
    Ok(parse_week(&records)?)
}

fn add_secs(totals: &mut Vec<ActivityTotal>, id: i32, number: &str, secs: i64) {
    match totals.iter_mut().find(|t| t.activity_id == id) {
        Some(t) => t.secs += secs,
        None => totals.push(ActivityTotal {
            activity_id: id,
            number: number.to_string(),
            secs,
        }),
    }
}

fn largest_first(totals: &mut [ActivityTotal]) {
    totals.sort_by(|a, b| b.secs.cmp(&a.secs).then(a.number.cmp(&b.number)));
}

/// Totals of the week. `queued` are local entries not uploaded yet; they count for their day.
pub fn summarize(week: &Week, queued: &[PendingEntry]) -> WeekSummary {
    let mut days = Vec::new();
    let mut activities: Vec<ActivityTotal> = Vec::new();

    for day in &week.days {
        let mut by_activity: Vec<ActivityTotal> = Vec::new();
        for e in &day.entries {
            add_secs(
                &mut by_activity,
                e.activity_id,
                &e.full_activity_number,
                e.duration,
            );
        }
        for q in queued.iter().filter(|q| q.date == day.date) {
            let (id, number) = q
                .activity
                .as_ref()
                .map_or((0, ""), |a| (a.id, a.full_number.as_str()));
            add_secs(&mut by_activity, id, number, q.duration);
        }
        largest_first(&mut by_activity);

        for t in &by_activity {
            add_secs(&mut activities, t.activity_id, &t.number, t.secs);
        }
        days.push(DaySummary {
            date: day.date,
            nominal_secs: day.nominal_secs,
            on_leave: day.on_leave,
            booked_secs: by_activity.iter().map(|t| t.secs).sum(),
            by_activity,
        });
    }
    largest_first(&mut activities);

    let target_secs: i64 = days.iter().map(|d| d.nominal_secs).sum();
    let booked_secs: i64 = days.iter().map(|d| d.booked_secs).sum();
    let leave_secs: i64 = days
        .iter()
        .filter(|d| d.on_leave)
        .map(|d| d.nominal_secs)
        .sum();
    let pensum_percent = days
        .iter()
        .map(|d| d.nominal_secs)
        .max()
        .filter(|max| *max > 0)
        .map(|max| ((max as f64 / DAILY_TARGET_SECS as f64) * 100.0).round() as u32);

    WeekSummary {
        target_secs,
        booked_secs,
        leave_secs,
        remaining_secs: target_secs - booked_secs - leave_secs,
        pensum_percent,
        activities,
        days,
    }
}

#[cfg(test)]
#[path = "week.tests.rs"]
mod tests;
