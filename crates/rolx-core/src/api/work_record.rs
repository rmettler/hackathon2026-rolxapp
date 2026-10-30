use chrono::Duration;
use serde::Deserialize;

use crate::date_range::DateRange;
use crate::session::Session;

fn duration_from_seconds<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Duration, D::Error> {
    Ok(Duration::seconds(i64::deserialize(d)?))
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordEntry {
    pub full_activity_number: String,
    #[serde(deserialize_with = "duration_from_seconds")]
    pub duration: Duration,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub date: String,
    pub entries: Vec<RecordEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkRecordError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The session is not, or no longer, accepted by the server.
    #[error("HTTP 401 Unauthorized")]
    Unauthorized,

    #[error("HTTP {0}")]
    Status(reqwest::StatusCode),
}

pub async fn get_range(
    http: &reqwest::Client,
    server_url: &str,
    session: &Session,
    range: DateRange,
) -> std::result::Result<Vec<Record>, WorkRecordError> {
    let url = format!(
        "{server_url}/api/v1/WorkRecord/{}/range/{range}",
        session.user_id
    );
    tracing::debug!(method = "GET", url, "HTTP request");

    let resp = http
        .get(&url)
        .bearer_auth(session.bearer_token.as_str())
        .send()
        .await?;
    let status = resp.status();
    let body = resp.text().await?;

    #[cfg(not(feature = "uncensored_logs"))]
    tracing::debug!(status = status.as_u16(), "HTTP response");
    #[cfg(feature = "uncensored_logs")]
    tracing::debug!(status = status.as_u16(), body, "HTTP response");

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(WorkRecordError::Unauthorized);
    }
    if !status.is_success() {
        return Err(WorkRecordError::Status(status));
    }

    Ok(serde_json::from_str(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape of `Records/WebApi/Resource/Record.cs` with its `RecordEntry.cs`.
    const RECORDS: &str = r#"[{
        "date": "2026-10-30",
        "userId": "00000000-0000-0000-0000-000000000001",
        "dayType": 0,
        "dayName": "",
        "nominalWorkTime": 28800,
        "paidLeaveType": null,
        "paidLeaveReason": null,
        "entries": [
            {
                "activityId": 1,
                "fullActivityNumber": "M0001.001.01",
                "duration": 5400,
                "begin": 28800,
                "pause": null,
                "comment": "Fake"
            },
            {
                "activityId": 2,
                "fullActivityNumber": "M0001.002.01",
                "duration": 60,
                "begin": null,
                "pause": null,
                "comment": null
            }
        ]
    }]"#;

    #[test]
    fn deserializes_records_with_durations_in_seconds() {
        let records: Vec<Record> = serde_json::from_str(RECORDS).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].date, "2026-10-30");
        assert_eq!(
            records[0].entries,
            [
                RecordEntry {
                    full_activity_number: "M0001.001.01".into(),
                    duration: Duration::minutes(90),
                    comment: Some("Fake".into()),
                },
                RecordEntry {
                    full_activity_number: "M0001.002.01".into(),
                    duration: Duration::minutes(1),
                    comment: None,
                },
            ]
        );
    }
}
