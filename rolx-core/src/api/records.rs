use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::client::RolxClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordEntry {
    pub activity_id: i32,
    pub full_activity_number: String,
    #[serde(with = "super::seconds")]
    pub duration: Duration,
    /// Start time in seconds since midnight.
    pub begin: Option<i32>,
    /// Pause duration in seconds.
    pub pause: Option<i32>,
    pub comment: Option<String>,
}

/// A work record for a single day. Enum fields stay numeric so a fetched record round-trips.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub date: String,
    pub user_id: String,
    pub day_type: i32,
    pub day_name: String,
    pub nominal_work_time: i64,
    pub paid_leave_type: Option<i32>,
    pub paid_leave_reason: Option<String>,
    pub entries: Vec<RecordEntry>,
}

fn iso(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

/// Fetches the records of the logged-in user for the half-open range `(begin..end]`.
pub async fn fetch_range(
    client: &RolxClient,
    begin: NaiveDate,
    end: NaiveDate,
) -> anyhow::Result<Vec<Record>> {
    client
        .get_json(&format!(
            "api/v1/WorkRecord/{}/range/{}..{}",
            client.session.user_id,
            iso(begin),
            iso(end)
        ))
        .await
}

/// Fetches the record of the logged-in user for a single day.
pub async fn fetch_record(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Option<Record>> {
    let records = fetch_range(client, day, day + Duration::days(1)).await?;
    Ok(records.into_iter().next())
}

/// Fetches the work entries of the logged-in user for a single day.
pub async fn fetch_day(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Vec<RecordEntry>> {
    Ok(fetch_record(client, day)
        .await?
        .map(|r| r.entries)
        .unwrap_or_default())
}

/// Replaces the record of its date on the server.
pub async fn put_record(client: &RolxClient, record: &Record) -> anyhow::Result<()> {
    client
        .put_json(
            &format!("api/v1/WorkRecord/{}/{}", record.user_id, record.date),
            record,
        )
        .await
}

/// Replaces the records of their dates on the server, all or none.
pub async fn bulk_update(client: &RolxClient, records: &[Record]) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct BulkRecord<'a> {
        records: &'a [Record],
    }

    let Some(first) = records.first() else {
        return Ok(());
    };
    client
        .put_json(
            &format!("api/v1/WorkRecord/{}/bulkupdate", first.user_id),
            &BulkRecord { records },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trips_server_json() {
        let json = r#"{"date":"2026-10-30","userId":"3f2b","dayType":0,"dayName":"Friday",
            "nominalWorkTime":28800,"paidLeaveType":null,"paidLeaveReason":null,
            "entries":[{"activityId":7,"fullActivityNumber":"P1-A1","duration":5400,
            "begin":32400,"pause":null,"comment":"review"}]}"#;
        let record: Record = serde_json::from_str(json).unwrap();
        assert_eq!(record.entries[0].duration, Duration::minutes(90));

        let value = serde_json::to_value(&record).unwrap();
        assert_eq!(value["entries"][0]["duration"], 5400);
        assert_eq!(value["dayType"], 0);
        assert_eq!(value["entries"][0]["begin"], 32400);
    }
}
