//! Uploads locally stamped entries to the server.
//!
//! The server replaces a whole day on update, so each day is fetched, extended and sent back.
//! Entries already present on the server are not added again, which makes retries safe.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{Local, NaiveDate};
use serde_json::{Value, json};

use crate::client::RolxClient;
use crate::stamp::{ActivityRef, PendingEntry, StampState};
use crate::{activities, records};

#[derive(Debug, Default)]
pub struct SyncReport {
    /// Entries that are now on the server.
    pub uploaded: usize,
    /// Entries kept locally because they have no activity yet.
    pub unassigned: usize,
    /// Days that could not be uploaded, with the reason. Their entries stay queued.
    pub failed: Vec<(NaiveDate, String)>,
}

fn entry_json(entry: &PendingEntry, activity: &ActivityRef) -> Value {
    json!({
        "activityId": activity.id,
        "fullActivityNumber": activity.full_number,
        "duration": entry.duration,
        "begin": entry.begin,
        "pause": (entry.pause > 0).then_some(entry.pause),
        "comment": entry.comment,
    })
}

/// Appends the entries to `record["entries"]`, skipping ones that already exist.
/// Returns how many were added.
fn merge_entries(record: &mut Value, entries: &[(&PendingEntry, &ActivityRef)]) -> usize {
    let Some(list) = record
        .as_object_mut()
        .map(|o| o.entry("entries").or_insert_with(|| json!([])))
        .and_then(Value::as_array_mut)
    else {
        return 0;
    };

    let mut added = 0;
    for (entry, activity) in entries {
        let exists = list.iter().any(|e| {
            e["activityId"] == activity.id
                && e["begin"] == entry.begin
                && e["duration"] == entry.duration
        });
        if !exists {
            list.push(entry_json(entry, activity));
            added += 1;
        }
    }
    added
}

async fn push_day(
    client: &RolxClient,
    day: NaiveDate,
    entries: &[(&PendingEntry, &ActivityRef)],
) -> anyhow::Result<()> {
    let mut record = records::fetch_day_record(client, day).await?;
    if merge_entries(&mut record, entries) > 0 {
        records::update_day_record(client, day, &record).await?;
    }
    Ok(())
}

/// Uploads all queued entries that have an activity. Entries of days that fail stay queued.
pub async fn run(client: &RolxClient, state_path: &Path) -> anyhow::Result<SyncReport> {
    let snapshot = StampState::load_from(state_path)?;
    let mut report = SyncReport::default();

    let mut by_day: BTreeMap<NaiveDate, Vec<(&PendingEntry, &ActivityRef)>> = BTreeMap::new();
    for entry in &snapshot.pending {
        match &entry.activity {
            Some(activity) => by_day
                .entry(entry.date)
                .or_default()
                .push((entry, activity)),
            None => report.unassigned += 1,
        }
    }

    let mut uploaded: Vec<&PendingEntry> = Vec::new();
    for (day, entries) in &by_day {
        match push_day(client, *day, entries).await {
            Ok(()) => uploaded.extend(entries.iter().map(|(e, _)| *e)),
            Err(e) => report.failed.push((*day, e.to_string())),
        }
    }
    report.uploaded = uploaded.len();

    if !uploaded.is_empty() {
        // Reload: the user may have stamped while the upload was running.
        let mut current = StampState::load_from(state_path)?;
        current.pending.retain(|e| !uploaded.contains(&e));
        current.save_to(state_path)?;
    }
    Ok(report)
}

/// Uploads the queue to the default location and refreshes the cached activity list.
pub async fn run_default(client: &RolxClient) -> anyhow::Result<SyncReport> {
    let report = run(client, &StampState::default_path()?).await?;
    if let Err(e) = activities::refresh(client, Local::now().date_naive()).await {
        tracing::warn!(error = %e, "could not refresh the activity list");
    }
    Ok(report)
}

#[cfg(test)]
#[path = "sync.tests.rs"]
mod tests;
