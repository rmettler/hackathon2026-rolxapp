use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::client::RolxClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: i32,
    pub number: i32,
    pub name: String,
    pub start_date: String,
    pub end_date: Option<String>,
    pub billability_id: i32,
    pub billability_name: String,
    pub is_billable: bool,
    pub is_over_budget: bool,
    pub is_over_planned: bool,
    pub budget: i64,
    pub planned: i64,
    pub actual: i64,
    pub budget_consumed_fraction: f64,
    pub planned_consumed_fraction: f64,
    pub project_name: String,
    pub subproject_name: String,
    pub customer_name: String,
    pub full_number: String,
    pub full_name: String,
    pub all_subproject_names: String,
}

/// Lists the activities still open at `date`.
pub async fn fetch_all(client: &RolxClient, date: NaiveDate) -> anyhow::Result<Vec<Activity>> {
    client
        .get_json(&format!(
            "api/v1/Activity?unlessEndedBeforeDate={}",
            date.format("%Y-%m-%d")
        ))
        .await
}

/// Lists the activities the logged-in user may book on `date`.
pub async fn fetch_suitable(client: &RolxClient, date: NaiveDate) -> anyhow::Result<Vec<Activity>> {
    client
        .get_json(&format!(
            "api/v1/Activity/suitable/{}/{}",
            client.session.user_id,
            date.format("%Y-%m-%d")
        ))
        .await
}

pub async fn fetch_favourites(client: &RolxClient) -> anyhow::Result<Vec<Activity>> {
    client.get_json("api/v1/activity/favourite").await
}

pub async fn add_favourite(client: &RolxClient, activity: &Activity) -> anyhow::Result<()> {
    client.put_json("api/v1/activity/favourite", activity).await
}

pub async fn remove_favourite(client: &RolxClient, activity: &Activity) -> anyhow::Result<()> {
    client
        .delete_json("api/v1/activity/favourite", activity)
        .await
}
