use chrono::NaiveDate;
use serde::Deserialize;

use crate::client::RolxClient;

#[derive(Debug, Deserialize)]
struct EditLock {
    date: String,
}

/// Fetches the date before which records are read-only.
pub async fn fetch(client: &RolxClient) -> anyhow::Result<NaiveDate> {
    let lock: EditLock = client.get_json("api/v1/edit-lock").await?;
    Ok(NaiveDate::parse_from_str(&lock.date, "%Y-%m-%d")?)
}
