use chrono::Duration;
use serde::{Deserialize, Serialize};

use crate::client::RolxClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub name: String,
    #[serde(with = "super::seconds")]
    pub duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemGroup {
    pub name: String,
    pub items: Vec<WorkItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserMonthReport {
    pub month: String,
    #[serde(with = "super::seconds")]
    pub overtime: Duration,
    #[serde(with = "super::seconds")]
    pub overtime_delta: Duration,
    #[serde(with = "super::seconds")]
    pub vacation: Duration,
    pub vacation_days: Option<f64>,
    #[serde(with = "super::seconds")]
    pub vacation_delta: Duration,
    pub vacation_delta_days: Option<f64>,
    #[serde(with = "super::seconds")]
    pub billable_time: Duration,
    #[serde(with = "super::seconds")]
    pub non_billable_time: Duration,
    #[serde(with = "super::seconds")]
    pub leave_time: Duration,
    pub work_item_groups: Option<Vec<WorkItemGroup>>,
}

/// Fetches the monthly report of the logged-in user; `month` is `yyyy-MM`.
pub async fn fetch(client: &RolxClient, month: &str) -> anyhow::Result<UserMonthReport> {
    client
        .get_json(&format!(
            "api/v1/user-report/{}/month/{month}",
            client.session.user_id
        ))
        .await
}
