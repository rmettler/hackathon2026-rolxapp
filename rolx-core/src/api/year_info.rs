use chrono::Duration;
use serde::Deserialize;

use crate::client::RolxClient;

#[derive(Debug, Clone, Deserialize)]
pub struct Holiday {
    pub name: String,
    pub date: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonthlyWorkTime {
    /// `yyyy-MM`.
    pub month: String,
    pub days: i32,
    /// Work time in seconds.
    hours: i64,
}

impl MonthlyWorkTime {
    pub fn work_time(&self) -> Duration {
        Duration::seconds(self.hours)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YearInfo {
    pub holidays: Vec<Holiday>,
    pub monthly_work_times: Vec<MonthlyWorkTime>,
}

pub async fn fetch(client: &RolxClient, year: i32) -> anyhow::Result<YearInfo> {
    client.get_json(&format!("api/v1/YearInfo/{year}")).await
}
