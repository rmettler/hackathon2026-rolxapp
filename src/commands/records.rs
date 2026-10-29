use anyhow::{anyhow, bail};
use chrono::{Duration, NaiveDate};
use serde::Deserialize;

use crate::client::RolxClient;

fn duration_from_seconds<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Duration, D::Error> {
    Ok(Duration::seconds(i64::deserialize(d)?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordEntry {
    full_activity_number: String,
    #[serde(deserialize_with = "duration_from_seconds")]
    duration: Duration,
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    entries: Vec<RecordEntry>,
}

fn fmt_duration(d: Duration) -> String {
    format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60)
}

pub async fn run(client: &RolxClient, date: Option<String>) -> anyhow::Result<()> {
    let day = match date {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map_err(|_| anyhow!("invalid date '{s}', expected yyyy-MM-dd"))?,
        None => chrono::Local::now().date_naive(),
    };

    let resp = {
        let url = format!(
            "{}/api/v1/WorkRecord/{}/range/{}..{}",
            client.server_url,
            client.session.user_id,
            day.format("%Y-%m-%d"),
            (day + Duration::days(1)).format("%Y-%m-%d"),
        );
        tracing::debug!(method = "GET", url, "HTTP request");
        client
            .http
            .get(&url)
            .bearer_auth(client.session.bearer_token.as_str())
            .send()
            .await?
    };

    let status = resp.status();
    let body = resp.text().await?;
    // TODO redact body?
    tracing::debug!(status = status.as_u16(), body, "HTTP response");

    if !status.is_success() {
        bail!("HTTP {status}");
    }

    let records: Vec<Record> = serde_json::from_str(&body)?;

    let entries: Vec<_> = records.into_iter().flat_map(|r| r.entries).collect();

    if entries.is_empty() {
        println!("No entries for {day}.");
        return Ok(());
    }

    for entry in &entries {
        let comment = entry.comment.as_deref().unwrap_or("");
        println!(
            "{:<20} {}  {}",
            entry.full_activity_number,
            fmt_duration(entry.duration),
            comment
        );
    }

    let total: Duration = entries.iter().map(|e| e.duration).sum();
    println!("{:<20} {}  total", "", fmt_duration(total));

    Ok(())
}
