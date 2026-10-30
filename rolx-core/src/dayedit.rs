//! Reading and saving the entries of one day on the server, safely.
//!
//! The server replaces a whole day on update. To not overwrite changes made elsewhere (web app, other
//! device), [`save_day`] only writes if the day still looks exactly like what the user was editing.

use std::path::Path;

use chrono::{Duration, Local, NaiveDate};
use serde::Deserialize;
use serde_json::Value;

use crate::client::RolxClient;
use crate::dayplan::{DayEntry, copyable, entries_of, set_entries, sort_entries};
use crate::records;
use crate::undo::{UndoRecord, UndoStore};

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("{0} is locked for editing (the lock is at {1})")]
    Locked(NaiveDate, NaiveDate),
    #[error("the day was changed in the meantime, reload it and try again")]
    Changed,
}

#[derive(Deserialize)]
struct EditLock {
    date: NaiveDate,
}

/// The first day that can still be edited; earlier days are read-only.
pub async fn fetch_edit_lock(client: &RolxClient) -> anyhow::Result<NaiveDate> {
    Ok(client.get_json::<EditLock>("api/v1/edit-lock").await?.date)
}

/// The entries of a day in chronological order (see [`sort_entries`]). Numbers shown to the user and
/// indices passed to [`save_day`] refer to this order.
pub async fn load_day(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Vec<DayEntry>> {
    let mut entries = entries_of(&records::fetch_day_record(client, day).await?)?;
    sort_entries(&mut entries);
    Ok(entries)
}

/// Replaces the entries of `day` with `new`, provided the server still has exactly `base`.
/// Only the entries are compared, not the order the server happens to list them in.
pub async fn save_day(
    client: &RolxClient,
    day: NaiveDate,
    base: &[DayEntry],
    new: &[DayEntry],
) -> anyhow::Result<()> {
    // If the lock cannot be read, let the server decide.
    if let Ok(lock) = fetch_edit_lock(client).await
        && day < lock
    {
        return Err(SaveError::Locked(day, lock).into());
    }

    let mut record = records::fetch_day_record(client, day).await?;
    let mut current = entries_of(&record)?;
    sort_entries(&mut current);
    let mut base = base.to_vec();
    sort_entries(&mut base);
    if current != base {
        return Err(SaveError::Changed.into());
    }

    let mut new = new.to_vec();
    sort_entries(&mut new);
    set_entries(&mut record, &new);
    records::update_day_record(client, day, &record).await
}

/// Like [`save_day`], and remembers the change so that [`undo_last`] can take it back.
pub async fn save_day_tracked(
    client: &RolxClient,
    day: NaiveDate,
    base: &[DayEntry],
    new: &[DayEntry],
    what: &str,
) -> anyhow::Result<()> {
    save_day_tracked_at(client, day, base, new, what, &UndoStore::default_path()?).await
}

pub async fn save_day_tracked_at(
    client: &RolxClient,
    day: NaiveDate,
    base: &[DayEntry],
    new: &[DayEntry],
    what: &str,
    undo_path: &Path,
) -> anyhow::Result<()> {
    save_day(client, day, base, new).await?;

    let mut before = base.to_vec();
    sort_entries(&mut before);
    // What the server now has, so that a later undo compares against the server's own spelling.
    let after = match load_day(client, day).await {
        Ok(entries) => entries,
        Err(_) => {
            let mut entries = new.to_vec();
            sort_entries(&mut entries);
            entries
        }
    };
    if before != after {
        let store = UndoStore {
            last: Some(UndoRecord {
                day,
                what: what.to_string(),
                before,
                after,
                at: Local::now().naive_local(),
            }),
        };
        // Failing to remember must not turn a successful save into an error.
        if let Err(e) = store.save_to(undo_path) {
            tracing::warn!("cannot remember the change for undo: {e:#}");
        }
    }
    Ok(())
}

/// Takes back the last tracked change, provided the day still is as that change left it.
pub async fn undo_last(client: &RolxClient) -> anyhow::Result<UndoRecord> {
    undo_last_at(client, &UndoStore::default_path()?).await
}

pub async fn undo_last_at(client: &RolxClient, undo_path: &Path) -> anyhow::Result<UndoRecord> {
    let record = UndoStore::load_from(undo_path)?
        .last
        .ok_or_else(|| anyhow::anyhow!("there is nothing to undo"))?;
    match save_day(client, record.day, &record.after, &record.before).await {
        Ok(()) => {}
        Err(e) if matches!(e.downcast_ref(), Some(SaveError::Changed)) => {
            anyhow::bail!(
                "cannot undo \"{}\": {} was changed since, undoing could lose that change",
                record.what,
                record.day
            );
        }
        Err(e) => return Err(e),
    }
    UndoStore::default().save_to(undo_path)?;
    Ok(record)
}

/// The most recent day before `day` (looking back at most `max_back` days) that has entries
/// worth repeating, with those entries.
pub fn latest_with_entries(
    records: &[Value],
) -> anyhow::Result<Option<(NaiveDate, Vec<DayEntry>)>> {
    let mut best: Option<(NaiveDate, Vec<DayEntry>)> = None;
    for record in records {
        let date: NaiveDate = serde_json::from_value(record["date"].clone())?;
        let entries = copyable(&entries_of(record)?);
        if !entries.is_empty() && best.as_ref().is_none_or(|(d, _)| date > *d) {
            best = Some((date, entries));
        }
    }
    Ok(best)
}

pub async fn previous_entries(
    client: &RolxClient,
    day: NaiveDate,
    max_back: i64,
) -> anyhow::Result<Option<(NaiveDate, Vec<DayEntry>)>> {
    let path = format!(
        "api/v1/WorkRecord/{}/range/{}..{}",
        client.session.user_id,
        (day - Duration::days(max_back)).format("%Y-%m-%d"),
        day.format("%Y-%m-%d"),
    );
    let records: Vec<Value> = client.get_json(&path).await?;
    latest_with_entries(&records)
}

#[cfg(test)]
#[path = "dayedit.tests.rs"]
mod tests;
