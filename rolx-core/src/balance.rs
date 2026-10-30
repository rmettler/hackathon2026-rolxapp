//! The server's running balance: flextime ("Gleitzeit") and remaining vacation ("Ferien").

use chrono::NaiveDate;
use serde_json::Value;

use crate::client::RolxClient;

#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    pub by_date: NaiveDate,
    /// Accumulated flextime; negative when the user owes time.
    pub overtime_secs: i64,
    pub vacation_available_secs: i64,
    pub vacation_available_days: Option<f64>,
    pub vacation_planned_secs: i64,
    pub vacation_planned_days: Option<f64>,
}

pub fn parse_balance(value: &Value) -> Result<Balance, serde_json::Error> {
    Ok(Balance {
        by_date: serde_json::from_value(value["byDate"].clone())?,
        overtime_secs: value["overtime"].as_i64().unwrap_or(0),
        vacation_available_secs: value["vacationAvailable"].as_i64().unwrap_or(0),
        vacation_available_days: value["vacationAvailableDays"].as_f64(),
        vacation_planned_secs: value["vacationPlanned"].as_i64().unwrap_or(0),
        vacation_planned_days: value["vacationPlannedDays"].as_f64(),
    })
}

/// The balance by the end of `day`.
pub async fn fetch_balance(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Balance> {
    let value: Value = client
        .get_json(&format!("api/v1/Balance/{}", day.format("%Y-%m-%d")))
        .await?;
    Ok(parse_balance(&value)?)
}

/// `+3:20`, `-1:30` or `0:00`; minutes are truncated.
fn fmt_signed_hm(secs: i64) -> String {
    let sign = match secs {
        s if s > 0 => "+",
        s if s < 0 => "-",
        _ => "",
    };
    let abs = secs.abs();
    format!("{sign}{}:{:02}", abs / 3600, abs / 60 % 60)
}

/// Days with up to two decimals and no trailing zeros: `12`, `12.5`, `0.25`.
fn fmt_days(days: f64) -> String {
    let text = format!("{days:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn fmt_amount(days: Option<f64>, secs: i64) -> String {
    match days {
        Some(d) => format!("{} {}", fmt_days(d), if d == 1.0 { "day" } else { "days" }),
        None => format!("{} h", crate::dayplan::fmt_hm(secs)),
    }
}

impl Balance {
    /// Flextime like `+3:20 h`.
    pub fn overtime_text(&self) -> String {
        format!("{} h", fmt_signed_hm(self.overtime_secs))
    }

    /// Remaining vacation like `12.5 days`, with planned vacation when there is some.
    pub fn vacation_text(&self) -> String {
        let available = fmt_amount(self.vacation_available_days, self.vacation_available_secs);
        if self.vacation_planned_secs > 0 {
            let planned = match self.vacation_planned_days {
                Some(d) => fmt_days(d),
                None => format!("{} h", crate::dayplan::fmt_hm(self.vacation_planned_secs)),
            };
            format!("{available} ({planned} planned)")
        } else {
            available
        }
    }
}

#[cfg(test)]
#[path = "balance.tests.rs"]
mod tests;
