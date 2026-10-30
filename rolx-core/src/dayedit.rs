//! Reading and saving the entries of one day on the server, safely.
//!
//! The server replaces a whole day on update. To not overwrite changes made elsewhere (web app, other
//! device), [`save_day`] only writes if the day still looks exactly like what the user was editing.

use chrono::NaiveDate;
use serde::Deserialize;

use crate::client::RolxClient;
use crate::dayplan::{DayEntry, entries_of, set_entries, sort_entries};
use crate::records;

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

#[cfg(test)]
#[path = "dayedit.tests.rs"]
mod tests;
