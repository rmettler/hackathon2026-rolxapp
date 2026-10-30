use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use serde_json::Value;

use crate::client::RolxClient;

/// Formats a duration as `h:mm`.
pub fn fmt_duration(d: Duration) -> String {
    format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60)
}

fn day_range_path(client: &RolxClient, day: NaiveDate) -> String {
    format!(
        "api/v1/WorkRecord/{}/range/{}..{}",
        client.session.user_id,
        day.format("%Y-%m-%d"),
        (day + Duration::days(1)).format("%Y-%m-%d"),
    )
}

/// Fetches the complete record of a day as the server sent it. The server replaces the whole
/// record on update, so edits must start from this and send everything back.
pub async fn fetch_day_record(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Value> {
    let date = day.format("%Y-%m-%d").to_string();
    let records: Vec<Value> = client.get_json(&day_range_path(client, day)).await?;
    records
        .into_iter()
        .find(|r| r["date"] == date.as_str())
        .ok_or_else(|| anyhow!("server returned no record for {date}"))
}

/// Replaces the record of a day with `record` (as obtained from [`fetch_day_record`]).
pub async fn update_day_record(
    client: &RolxClient,
    day: NaiveDate,
    record: &Value,
) -> anyhow::Result<()> {
    client
        .put_json(
            &format!(
                "api/v1/WorkRecord/{}/{}",
                client.session.user_id,
                day.format("%Y-%m-%d")
            ),
            record,
        )
        .await
}
