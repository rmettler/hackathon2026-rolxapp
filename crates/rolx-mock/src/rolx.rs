use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc, Weekday};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;

/// The only user the mock knows.
pub const MOCK_USER_ID: &str = "00000000-0000-4000-8000-000000000001";

const TOKEN_PREFIX: &str = "mock-bearer-";

/// Tokens carry their expiry, so a session can be seeded without talking to the mock first.
pub fn bearer_token_expiring_at(expires: DateTime<Utc>) -> String {
    format!("{TOKEN_PREFIX}{}", expires.timestamp())
}

/// Checks the `Authorization` header. Like the real server, rejects with an empty 401 body.
fn authorize(headers: &HeaderMap) -> Result<(), StatusCode> {
    let expires = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|token| token.strip_prefix(TOKEN_PREFIX))
        .and_then(|timestamp| timestamp.parse().ok())
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if expires < Utc::now() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(())
}

fn approval(state: &AppState) -> Value {
    let expires = Utc::now() + state.config.token_lifetime;
    json!({
        "user": {
            "id": MOCK_USER_ID,
            "firstName": "Mock",
            "lastName": "User",
            "email": "mock.user@example.invalid",
            "avatarUrl": "",
            "role": 1,
            "entryDate": "2020-01-01",
            "leavingDate": null,
            "isConfirmed": true,
            "partTimeSettings": [],
            "balanceCorrections": [],
            "vacationDaysSettings": [],
            "targetBillabilitySettings": [],
            "positionSettings": [],
        },
        "bearerToken": bearer_token_expiring_at(expires),
        "expires": expires.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    })
}

pub async fn sign_in_info() -> Json<Value> {
    Json(json!({
        "googleClientId": "mock-client-id",
        "installationId": "mock-installation",
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInData {
    google_id_token: String,
}

pub async fn sign_in(State(state): State<AppState>, Json(data): Json<SignInData>) -> Response {
    if !data.google_id_token.starts_with("mock-") {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(approval(&state)).into_response()
}

pub async fn extend(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match authorize(&headers) {
        Ok(()) => Json(approval(&state)).into_response(),
        Err(status) => status.into_response(),
    }
}

pub async fn work_record_range(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, range)): Path<(String, String)>,
) -> Response {
    if let Err(status) = authorize(&headers) {
        return status.into_response();
    }
    tokio::time::sleep(state.config.work_record_delay).await;
    let Some((begin, end)) = parse_range(&range) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !user_id.eq_ignore_ascii_case(MOCK_USER_ID) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let records: Vec<Value> = begin
        .iter_days()
        .take_while(|day| *day < end)
        .map(record)
        .collect();
    Json(records).into_response()
}

fn parse_range(range: &str) -> Option<(NaiveDate, NaiveDate)> {
    let (begin, end) = range.split_once("..")?;
    Some((parse_date(begin)?, parse_date(end)?))
}

fn parse_date(date: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// Deterministic record of a day: none on weekends, otherwise two or three entries depending on
/// the date, so that switching days visibly changes the content.
fn record(day: NaiveDate) -> Value {
    let is_weekend = matches!(day.weekday(), Weekday::Sat | Weekday::Sun);
    let entries = if is_weekend {
        Vec::new()
    } else {
        let mut entries = vec![
            entry(
                1,
                "M0001.001.01",
                Duration::hours(4) + Duration::minutes(i64::from(day.day())),
                Some(format!("Mock work on {day}")),
            ),
            entry(2, "M0001.002.01", Duration::hours(3), None),
        ];
        if day.ordinal().is_multiple_of(2) {
            entries.push(entry(
                3,
                "M0002.001.01",
                Duration::minutes(45),
                Some("Mock meeting".to_string()),
            ));
        }
        entries
    };

    json!({
        "date": day.format("%Y-%m-%d").to_string(),
        "userId": MOCK_USER_ID,
        "dayType": if is_weekend { 1 } else { 0 },
        "dayName": day.weekday().to_string(),
        "nominalWorkTime": if is_weekend { 0 } else { 8 * 3600 },
        "paidLeaveType": null,
        "paidLeaveReason": null,
        "entries": entries,
    })
}

fn entry(activity_id: u32, number: &str, duration: Duration, comment: Option<String>) -> Value {
    json!({
        "activityId": activity_id,
        "fullActivityNumber": number,
        "duration": duration.num_seconds(),
        "begin": null,
        "pause": null,
        "comment": comment,
    })
}
