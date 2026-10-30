//! Reading and saving the entries of one day on the server, safely.
//!
//! The server replaces a whole day on update. To not overwrite changes made elsewhere (web app, other
//! device), [`save_day`] only writes if the day still looks exactly like what the user was editing.

use chrono::NaiveDate;
use serde::Deserialize;

use crate::client::RolxClient;
use crate::dayplan::{DayEntry, entries_of, set_entries};
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

pub async fn load_day(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Vec<DayEntry>> {
    Ok(entries_of(&records::fetch_day_record(client, day).await?)?)
}

/// Replaces the entries of `day` with `new`, provided the server still has exactly `base`.
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
    if entries_of(&record)? != base {
        return Err(SaveError::Changed.into());
    }
    set_entries(&mut record, new);
    records::update_day_record(client, day, &record).await
}

#[cfg(test)]
#[path = "dayedit.tests.rs"]
mod tests;
