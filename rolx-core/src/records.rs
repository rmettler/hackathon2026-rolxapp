use anyhow::bail;
use chrono::{Duration, NaiveDate};
use serde::Deserialize;

use crate::client::RolxClient;

fn duration_from_seconds<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Duration, D::Error> {
    Ok(Duration::seconds(i64::deserialize(d)?))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordEntry {
    pub full_activity_number: String,
    #[serde(deserialize_with = "duration_from_seconds")]
    pub duration: Duration,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub entries: Vec<RecordEntry>,
}

/// Formats a duration as `h:mm`.
pub fn fmt_duration(d: Duration) -> String {
    format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60)
}

/// Fetches the work entries of the logged-in user for a single day.
pub async fn fetch_day(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Vec<RecordEntry>> {
    let url = format!(
        "{}/api/v1/WorkRecord/{}/range/{}..{}",
        client.server_url,
        client.session.user_id,
        day.format("%Y-%m-%d"),
        (day + Duration::days(1)).format("%Y-%m-%d"),
    );
    tracing::debug!(method = "GET", url, "HTTP request");
    let resp = client
        .http
        .get(&url)
        .bearer_auth(client.session.bearer_token.as_str())
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;
    // TODO redact body?
    tracing::debug!(status = status.as_u16(), body, "HTTP response");

    if !status.is_success() {
        bail!("HTTP {status}");
    }

    let records: Vec<Record> = serde_json::from_str(&body)?;
    Ok(records.into_iter().flat_map(|r| r.entries).collect())
}
