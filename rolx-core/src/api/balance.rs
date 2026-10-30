use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::client::RolxClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub by_date: String,
    #[serde(with = "super::seconds")]
    pub overtime: Duration,
    #[serde(with = "super::seconds")]
    pub vacation_available: Duration,
    pub vacation_available_days: Option<f64>,
    #[serde(with = "super::seconds")]
    pub vacation_planned: Duration,
    pub vacation_planned_days: Option<f64>,
}

pub async fn fetch(client: &RolxClient, date: NaiveDate) -> anyhow::Result<Balance> {
    client
        .get_json(&format!("api/v1/Balance/{}", date.format("%Y-%m-%d")))
        .await
}
