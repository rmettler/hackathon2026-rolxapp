//! What the day view shows, apart from how it looks.

use chrono::{Duration, NaiveDate};
use rolx_core::api::sign_in::SignInExtensionError;
use rolx_core::api::work_record::{self, Record, WorkRecordError, format_duration};
use rolx_core::client::{SessionError, ensure_fresh};
use rolx_core::date_range::DateRange;
use rolx_core::session::Session;

use crate::platform::PlatformHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRow {
    pub activity: String,
    pub duration: String,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayEntries {
    pub rows: Vec<EntryRow>,
    pub total: String,
}

impl DayEntries {
    pub fn from_records(records: &[Record]) -> Self {
        let entries = || records.iter().flat_map(|record| &record.entries);
        let total: Duration = entries().map(|entry| entry.duration).sum();
        Self {
            rows: entries()
                .map(|entry| EntryRow {
                    activity: entry.full_activity_number.clone(),
                    duration: format_duration(entry.duration),
                    comment: entry.comment.clone().unwrap_or_default(),
                })
                .collect(),
            total: format_duration(total),
        }
    }
}

const NOT_CONFIGURED: &str = "The server is not configured.";
const UNREACHABLE: &str = "The server could not be reached.";
const UNEXPECTED_ANSWER: &str = "The server gave an unexpected answer.";
const NOT_SAVED: &str = "The session could not be saved.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The session is gone or not accepted: signing in again is the only way on.
    SessionRejected,
    /// Anything else, such as a network problem: trying again may help.
    Failed(&'static str),
}

/// A failure the user can retry. The fixed text goes to the log as well: the errors themselves are
/// not logged, as they can quote the server URL, the user id or parts of the answer.
fn failed(message: &'static str) -> LoadError {
    tracing::warn!(message, "could not load");
    LoadError::Failed(message)
}

fn session_error(error: SessionError) -> LoadError {
    match error {
        SessionError::NoSession | SessionError::Expired => LoadError::SessionRejected,
        SessionError::Extension(SignInExtensionError::Http(_)) => failed(UNREACHABLE),
        SessionError::Extension(_) => failed(UNEXPECTED_ANSWER),
        SessionError::Load(_) | SessionError::Save(_) => failed(NOT_SAVED),
    }
}

fn work_record_error(error: WorkRecordError) -> LoadError {
    match error {
        WorkRecordError::Unauthorized => LoadError::SessionRejected,
        WorkRecordError::Http(_) => failed(UNREACHABLE),
        WorkRecordError::Json(_) | WorkRecordError::Status(_) => failed(UNEXPECTED_ANSWER),
    }
}

/// The session to work with: extended and saved first if near expiry.
pub async fn fresh_session(
    platform: &PlatformHandle,
    session: Session,
) -> Result<Session, LoadError> {
    let server_url = platform
        .server_url()
        .ok_or_else(|| failed(NOT_CONFIGURED))?;
    ensure_fresh(
        &platform.http_client(),
        &server_url,
        session,
        platform.session_store(),
    )
    .await
    .map_err(session_error)
}

/// The entries of `date`, using a session known to be fresh.
pub async fn load_day(
    platform: &PlatformHandle,
    session: &Session,
    date: NaiveDate,
) -> Result<DayEntries, LoadError> {
    let server_url = platform
        .server_url()
        .ok_or_else(|| failed(NOT_CONFIGURED))?;
    let records = work_record::get_range(
        &platform.http_client(),
        &server_url,
        session,
        DateRange::day(date),
    )
    .await
    .map_err(work_record_error)?;
    Ok(DayEntries::from_records(&records))
}

#[cfg(test)]
mod tests {
    use rolx_core::api::work_record::RecordEntry;

    use super::*;

    fn entry(activity: &str, minutes: i64, comment: Option<&str>) -> RecordEntry {
        RecordEntry {
            full_activity_number: activity.to_string(),
            duration: Duration::minutes(minutes),
            comment: comment.map(str::to_string),
        }
    }

    fn record(entries: Vec<RecordEntry>) -> Record {
        Record {
            date: "2026-10-30".to_string(),
            entries,
        }
    }

    #[test]
    fn rows_and_total_cover_the_entries_of_all_records() {
        let records = [
            record(vec![entry("A", 90, Some("Fake")), entry("B", 5, None)]),
            record(vec![entry("C", 400, None)]),
        ];

        let day = DayEntries::from_records(&records);

        assert_eq!(
            day.rows,
            [
                EntryRow {
                    activity: "A".into(),
                    duration: "1:30".into(),
                    comment: "Fake".into(),
                },
                EntryRow {
                    activity: "B".into(),
                    duration: "0:05".into(),
                    comment: String::new(),
                },
                EntryRow {
                    activity: "C".into(),
                    duration: "6:40".into(),
                    comment: String::new(),
                },
            ]
        );
        assert_eq!(day.total, "8:15");
    }

    #[test]
    fn no_entries_give_no_rows_and_a_zero_total() {
        let day = DayEntries::from_records(&[record(Vec::new())]);
        assert!(day.rows.is_empty());
        assert_eq!(day.total, "0:00");
    }
}
