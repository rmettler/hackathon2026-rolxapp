//! Working without a connection: the local copy, pending changes and uploading them later.

use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::{Datelike, Duration, Local, NaiveDate};
use rolx_core::api::activity::Activity;
use rolx_core::api::edit_lock;
use rolx_core::api::records::{self, Record};
use rolx_core::client::{ApiError, RolxClient, is_offline, is_unauthorized};

use super::activities::is_bookable;
use super::{App, DayView, Msg, StampPart};
use crate::copy::{LocalCopy, Stamped, first_of_month, month_file, record_date, window};
use crate::outbox::{ChangeState, Outbox, PendingChange, days_equal};
use crate::store::LocalStore;
use crate::time::week_start;

pub(crate) const NOT_OFFLINE: &str = "Not available offline, connect once to load this day";
pub(crate) const NO_COPY_YET: &str = "No local copy yet, connect once to download your data";

/// Something that is loaded from the server, or from the local copy when there is no connection.
pub enum Load {
    Header,
    Day(NaiveDate),
    /// The week starting on this Monday.
    Week(NaiveDate),
    Activities,
    Year(i32),
    Report(String),
    /// The month starting on this day.
    Charts(NaiveDate),
    StampDays(Vec<StampPart>),
}

/// What happened to one pending change when syncing.
pub enum SyncOutcome {
    /// Uploaded.
    Uploaded,
    /// The server already holds the change; carries the server's day.
    AlreadySynced(Record),
    /// The server's day changed meanwhile; carries it.
    Conflict(Record),
    Rejected(String),
    Offline,
    Unauthorized,
}

fn outcome_of(error: &anyhow::Error) -> SyncOutcome {
    if is_offline(error) {
        SyncOutcome::Offline
    } else if is_unauthorized(error) {
        SyncOutcome::Unauthorized
    } else {
        SyncOutcome::Rejected(match error.downcast_ref::<ApiError>() {
            Some(ApiError::Rejected { message, .. }) => message.clone(),
            _ => format!("{error:#}"),
        })
    }
}

async fn sync_one(client: &RolxClient, change: &PendingChange) -> SyncOutcome {
    let server = match records::fetch_record(client, change.date).await {
        Ok(Some(server)) => server,
        Ok(None) => return SyncOutcome::Rejected("The server has no record for this day".into()),
        Err(error) => return outcome_of(&error),
    };
    if days_equal(&server, &change.record) {
        return SyncOutcome::AlreadySynced(server);
    }
    if !days_equal(&server, &change.base) {
        return SyncOutcome::Conflict(server);
    }
    match records::put_record(client, &change.record).await {
        Ok(()) => SyncOutcome::Uploaded,
        Err(error) => outcome_of(&error),
    }
}

impl App {
    /// Takes the local data from `store` and keeps writing to it.
    pub fn install_store(&mut self, store: LocalStore) {
        let (mut copy, mut problems) = LocalCopy::load(&store);
        for month in copy.prune(Local::now().date_naive()) {
            let _ = store.remove(&month_file(month));
        }
        self.copy = copy;
        match Outbox::load(&store) {
            Ok(outbox) => self.outbox = outbox,
            Err(error) => {
                // Keep the unreadable file instead of overwriting it with an empty outbox.
                let damaged = store.root().join("outbox.damaged.json");
                let _ = std::fs::rename(store.root().join("outbox.json"), &damaged);
                problems.push(format!(
                    "Pending changes unreadable, kept as {}: {error}",
                    damaged.display()
                ));
            }
        }
        if !problems.is_empty() {
            self.status = Some(problems.join("; "));
        }
        self.store = Some(store);
    }

    /// The client when the server can be tried: connected, not known to be offline, logged in.
    pub(crate) fn reachable(&self) -> Option<Arc<RolxClient>> {
        self.client
            .clone()
            .filter(|_| self.online && !self.needs_login)
    }

    /// Notes a failed request; returns whether the cause was the connection or the login,
    /// in which case the local copy takes over.
    pub(crate) fn note_failure(&mut self, error: &anyhow::Error) -> bool {
        if is_offline(error) {
            self.online = false;
            true
        } else if is_unauthorized(error) {
            self.needs_login = true;
            self.status = Some("Session expired, press L to log in".into());
            true
        } else {
            false
        }
    }

    pub(crate) fn persist_outbox(&self) -> Result<(), String> {
        match &self.store {
            Some(store) => self.outbox.save(store),
            None => Ok(()),
        }
    }

    pub(crate) fn persist_months(&mut self, months: impl IntoIterator<Item = NaiveDate>) {
        let result = match &self.store {
            Some(store) => self.copy.save_months(store, months),
            None => Ok(()),
        };
        if let Err(error) = result {
            self.status = Some(format!("Can't save the local copy: {error}"));
        }
    }

    fn persist_reference(&mut self) {
        let result = match &self.store {
            Some(store) => self.copy.save_reference(store),
            None => Ok(()),
        };
        if let Err(error) = result {
            self.status = Some(format!("Can't save the local copy: {error}"));
        }
    }

    /// Keeps days the server holds in the local copy.
    pub(crate) fn learn_days(&mut self, records: &[Record]) {
        let months = self.copy.put_days(records);
        self.persist_months(months);
    }

    /// Keeps a whole month the server returned in the local copy.
    pub(crate) fn learn_month(&mut self, month: NaiveDate, records: &[Record]) {
        self.copy.put_month(month, records, Local::now());
        self.persist_months([month]);
    }

    pub(crate) fn learn_header(
        &mut self,
        balance: &rolx_core::api::balance::Balance,
        lock: NaiveDate,
    ) {
        let reference = &mut self.copy.reference;
        reference.balance = Some(Stamped {
            at: Local::now(),
            value: balance.clone(),
        });
        reference.edit_lock = Some(lock);
        self.persist_reference();
    }

    pub(crate) fn learn_activities(&mut self, all: &[Activity], favourites: &[Activity]) {
        self.copy.reference.activities = all.to_vec();
        self.copy.reference.favourites = favourites.to_vec();
        self.persist_reference();
    }

    pub(crate) fn learn_year(&mut self, year: i32, info: &rolx_core::api::year_info::YearInfo) {
        self.copy.reference.year_infos.insert(
            year,
            Stamped {
                at: Local::now(),
                value: info.clone(),
            },
        );
        self.persist_reference();
    }

    pub(crate) fn learn_report(
        &mut self,
        month: &str,
        report: &rolx_core::api::user_report::UserMonthReport,
    ) {
        self.copy.reference.reports.insert(
            month.to_string(),
            Stamped {
                at: Local::now(),
                value: report.clone(),
            },
        );
        self.persist_reference();
    }

    fn suitable_from_copy(&self, day: NaiveDate) -> Vec<Activity> {
        self.copy
            .reference
            .activities
            .iter()
            .filter(|a| is_bookable(a, day))
            .cloned()
            .collect()
    }

    /// A load failed on the server; fall back to the copy when the cause was the connection.
    pub(crate) fn on_load_failed(&mut self, load: Load, error: anyhow::Error) {
        if self.note_failure(&error) {
            self.serve_from_copy(load);
        } else {
            self.status = Some(format!("{error:#}"));
        }
    }

    /// Applies `load` using what the local copy holds.
    pub(crate) fn serve_from_copy(&mut self, load: Load) {
        match load {
            Load::Header => {
                let reference = &self.copy.reference;
                let balance = reference.balance.as_ref().map(|b| (b.value.clone(), b.at));
                let lock = reference.edit_lock;
                if let Some((balance, at)) = balance {
                    self.balance = Some(balance);
                    self.balance_at = Some(at);
                }
                if lock.is_some() {
                    self.edit_lock = lock;
                }
            }
            Load::Day(day) => {
                let record = self.copy.day(day).cloned();
                if record.is_none() && day == self.day {
                    self.status = Some(if self.copy.is_empty() {
                        NO_COPY_YET.into()
                    } else {
                        NOT_OFFLINE.into()
                    });
                }
                let suitable = self.suitable_from_copy(day);
                self.apply_day(day, record, suitable);
            }
            Load::Week(monday) => {
                let records = self.copy.days_between(monday, monday + Duration::days(7));
                let suitable = self.suitable_from_copy(self.day);
                self.apply_week(monday, records, suitable);
            }
            Load::Activities => {
                let reference = &self.copy.reference;
                let (all, favourites) =
                    (reference.activities.clone(), reference.favourites.clone());
                self.apply_activities(all, favourites);
            }
            Load::Year(year) => {
                if let Some(info) = self.copy.reference.year_infos.get(&year) {
                    let (info, at) = (info.value.clone(), info.at);
                    self.apply_year(year, info);
                    self.year_at = Some(at);
                }
            }
            Load::Report(month) => {
                if let Some(report) = self.copy.reference.reports.get(&month) {
                    let (report, at) = (report.value.clone(), report.at);
                    self.apply_report(month, report);
                    self.report_at = Some(at);
                }
            }
            Load::Charts(first) => {
                let next = (first + Duration::days(32)).with_day(1).unwrap_or(first);
                let records = self.copy.days_between(first, next);
                self.apply_charts(first, records);
            }
            Load::StampDays(parts) => {
                let mut records = Vec::new();
                for part in &parts {
                    match self.copy.day(part.date) {
                        Some(record) => records.push(self.outbox.overlay(record.clone())),
                        None => {
                            self.status = Some(format!(
                                "{} is not available offline; the stamp keeps running",
                                part.date.format("%a %d.%m.")
                            ));
                            return;
                        }
                    }
                }
                self.open_stamp_form(parts, records);
            }
        }
    }

    pub(crate) fn apply_day(
        &mut self,
        day: NaiveDate,
        record: Option<Record>,
        suitable: Vec<Activity>,
    ) {
        if day != self.day || self.day_view != DayView::Day {
            return;
        }
        self.record = record.map(|r| self.outbox.overlay(r));
        self.suitable = suitable;
        self.sort_suitable();
        self.sel_entry = self.sel_entry.min(self.day_row_count().saturating_sub(1));
    }

    pub(crate) fn apply_week(
        &mut self,
        monday: NaiveDate,
        records: Vec<Record>,
        suitable: Vec<Activity>,
    ) {
        if self.day_view != DayView::Week || monday != week_start(self.day) {
            return;
        }
        self.week = records
            .into_iter()
            .map(|r| self.outbox.overlay(r))
            .collect();
        self.suitable = suitable;
        self.sort_suitable();
        self.sync_week_record();
        let rows = self.week_grid().rows.len();
        self.sel_row = self.sel_row.min(rows.saturating_sub(1));
    }

    pub(crate) fn apply_activities(&mut self, mut all: Vec<Activity>, favourites: Vec<Activity>) {
        self.favourites = favourites.iter().map(|a| a.id).collect();
        self.favourite_activities = favourites;
        all.sort_by_key(|a| !self.favourites.contains(&a.id));
        self.sel_activity = self.sel_activity.min(all.len().saturating_sub(1));
        self.activities = all;
        self.sort_suitable();
        self.sel_entry = self.sel_entry.min(self.day_row_count().saturating_sub(1));
        let rows = self.week_grid().rows.len();
        self.sel_row = self.sel_row.min(rows.saturating_sub(1));
    }

    pub(crate) fn apply_year(&mut self, year: i32, info: rolx_core::api::year_info::YearInfo) {
        if year == self.year {
            self.year_info = Some(info);
        }
    }

    pub(crate) fn apply_report(
        &mut self,
        month: String,
        report: rolx_core::api::user_report::UserMonthReport,
    ) {
        if month == self.month_key() {
            self.report = Some(report);
        }
    }

    pub(crate) fn apply_charts(&mut self, month: NaiveDate, records: Vec<Record>) {
        if month != self.chart_month {
            return;
        }
        self.chart_records = Some(
            records
                .into_iter()
                .map(|r| self.outbox.overlay(r))
                .collect(),
        );
        self.settle_chart_selection();
    }

    /// Each day as it was before the change: the local copy, else what is shown.
    fn bases_for(&self, records: &[Record]) -> Vec<Option<Record>> {
        records
            .iter()
            .map(|record| {
                let date = record_date(record)?;
                self.copy
                    .day(date)
                    .or_else(|| self.record.as_ref().filter(|r| r.date == record.date))
                    .or_else(|| self.week.iter().find(|r| r.date == record.date))
                    .cloned()
            })
            .collect()
    }

    fn has_pending(&self, record: &Record) -> bool {
        record_date(record).is_some_and(|date| self.outbox.get(date).is_some())
    }

    /// Saves one day: to the server when it can be reached, else as a pending change.
    pub(crate) fn put(&mut self, record: Record) {
        self.save_days(vec![record], false);
    }

    /// Saves days, all or none. `stamp` marks the booking of a stamp.
    pub(crate) fn save_days(&mut self, records: Vec<Record>, stamp: bool) {
        let bases = self.bases_for(&records);
        // A day that already has pending changes is only updated in the outbox, so the
        // server is not asked about a day whose base it has moved past.
        let any_pending = records.iter().any(|r| self.has_pending(r));
        let Some(client) = self.reachable().filter(|_| !any_pending) else {
            return self.queue_days(records, bases, stamp);
        };
        // Shown before the server answers, which can take a while without a connection.
        for record in &records {
            self.show(record);
        }
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = match records.as_slice() {
                [record] if !stamp => records::put_record(&client, record).await,
                _ => records::bulk_update(&client, &records).await,
            };
            let msg = match result {
                Ok(()) => {
                    let _ = tx.send(Msg::Learned(records.clone()));
                    if stamp {
                        Msg::StampBooked(records.len())
                    } else {
                        Msg::Saved
                    }
                }
                Err(error) if is_offline(&error) || is_unauthorized(&error) => Msg::SaveOffline {
                    records,
                    bases,
                    stamp,
                    error,
                },
                Err(error) => Msg::SaveFailed(match error.downcast_ref::<ApiError>() {
                    Some(ApiError::Rejected { message, .. }) => message.clone(),
                    _ => format!("{error:#}"),
                }),
            };
            let _ = tx.send(msg);
        });
    }

    /// Keeps days as pending changes; all or none.
    pub(crate) fn queue_days(
        &mut self,
        records: Vec<Record>,
        bases: Vec<Option<Record>>,
        stamp: bool,
    ) {
        for (record, base) in records.iter().zip(&bases) {
            if base.is_none() && !self.has_pending(record) {
                let label = record_date(record).map_or(record.date.clone(), |d| d.to_string());
                self.status = Some(format!("{label}: {NOT_OFFLINE}"));
                return;
            }
        }
        let now = Local::now();
        let count = records.len();
        let saved = records.clone();
        for (record, base) in records.into_iter().zip(bases) {
            let Some(date) = record_date(&record) else {
                continue;
            };
            // The base only matters for a day's first pending change, which always has one.
            let base = base.unwrap_or_else(|| record.clone());
            self.outbox.record_change(&base, record, date, now);
        }
        self.status = Some(match self.persist_outbox() {
            Err(error) => format!("Can't save the pending changes: {error}"),
            Ok(()) if stamp => {
                format!("Booked {count} offline, will upload when connected")
            }
            Ok(()) => "Saved offline, will upload when connected".into(),
        });
        if stamp {
            let status = self.status.take();
            self.clear_stamp();
            self.status = status;
        }
        for record in &saved {
            self.show(record);
        }
        if self.reachable().is_some() {
            self.start_sync();
        }
    }

    /// Replaces the shown copies of a day, so a change is visible before any reload.
    pub(crate) fn show(&mut self, record: &Record) {
        for shown in self
            .record
            .iter_mut()
            .chain(self.week.iter_mut())
            .chain(self.chart_records.iter_mut().flatten())
        {
            if shown.date == record.date {
                *shown = record.clone();
            }
        }
        self.sel_entry = self.sel_entry.min(self.day_row_count().saturating_sub(1));
    }

    /// Uploads the pending changes that wait for a connection.
    pub(crate) fn start_sync(&mut self) {
        if self.syncing {
            return;
        }
        let Some(client) = self.reachable() else {
            return;
        };
        let work: Vec<PendingChange> = self
            .outbox
            .upload_order()
            .into_iter()
            .filter_map(|date| self.outbox.get(date).cloned())
            .collect();
        if work.is_empty() {
            return;
        }
        self.syncing = true;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut outcomes = Vec::new();
            for change in &work {
                let outcome = sync_one(&client, change).await;
                let stop = matches!(outcome, SyncOutcome::Offline | SyncOutcome::Unauthorized);
                outcomes.push((change.date, outcome));
                if stop {
                    break;
                }
            }
            let _ = tx.send(Msg::SyncDone(outcomes));
        });
    }

    pub(crate) fn finish_sync(&mut self, outcomes: Vec<(NaiveDate, SyncOutcome)>) {
        self.syncing = false;
        let mut uploaded = 0;
        let mut months = BTreeSet::new();
        for (date, outcome) in outcomes {
            match outcome {
                SyncOutcome::Uploaded => {
                    if let Some(change) = self.outbox.remove(date) {
                        months.extend(self.copy.put_days(&[change.record]));
                        uploaded += 1;
                    }
                }
                SyncOutcome::AlreadySynced(server) => {
                    self.outbox.remove(date);
                    months.extend(self.copy.put_days(&[server]));
                }
                SyncOutcome::Conflict(server) => {
                    if let Some(change) = self.outbox.get_mut(date) {
                        change.state = ChangeState::Conflict;
                        change.server = Some(server);
                        change.message = None;
                    }
                }
                SyncOutcome::Rejected(message) => {
                    if let Some(change) = self.outbox.get_mut(date) {
                        change.state = ChangeState::Rejected;
                        change.message = Some(message);
                    }
                }
                SyncOutcome::Offline => self.online = false,
                SyncOutcome::Unauthorized => {
                    self.needs_login = true;
                    self.status = Some("Session expired, press L to log in and upload".into());
                }
            }
        }
        if let Err(error) = self.persist_outbox() {
            self.status = Some(format!("Can't save the pending changes: {error}"));
        }
        self.persist_months(months);
        if uploaded > 0 && !self.needs_login {
            self.status = Some(format!(
                "Uploaded {uploaded} pending {}",
                if uploaded == 1 { "change" } else { "changes" }
            ));
        }
        let attention = self.outbox.needs_attention();
        if attention > 0 && !self.needs_login {
            let note = format!("{attention} pending change(s) need attention, press O");
            self.status = Some(match self.status.take() {
                Some(status) => format!("{status}; {note}"),
                None => note,
            });
        }
        self.load_view();
        self.load_header();
    }

    /// Time to retry: probe the server while offline, upload while online.
    pub(crate) fn on_sync_tick(&mut self) {
        if self.client.is_none() || self.needs_login {
            return;
        }
        if self.online {
            self.start_sync();
        } else {
            self.probe();
        }
    }

    fn probe(&self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match edit_lock::fetch(&client).await {
                Ok(_) => Msg::Online,
                Err(error) => Msg::ProbeFailed(error),
            };
            let _ = tx.send(msg);
        });
    }

    /// The server is reachable again.
    pub(crate) fn on_online(&mut self) {
        self.online = true;
        self.status = Some("Back online".into());
        self.load_header();
        self.load_view();
        self.load_activities();
        self.load_year();
        self.load_report();
        self.start_sync();
        self.start_download();
    }

    /// Refreshes the local copy: the months never fetched, and every month from the edit lock on,
    /// since only those can still change. The current month comes first.
    pub(crate) fn start_download(&mut self) {
        if self.download.is_some() {
            return;
        }
        let Some(client) = self.reachable() else {
            return;
        };
        let today = Local::now().date_naive();
        let current = first_of_month(today);
        let from = self
            .edit_lock
            .or(self.copy.reference.edit_lock)
            .map_or(current, first_of_month);
        let mut months: Vec<NaiveDate> = window(today)
            .into_iter()
            .filter(|m| self.copy.month_fetched(*m).is_none() || *m >= from)
            .collect();
        months.sort_by_key(|m| (*m != current, std::cmp::Reverse(*m)));
        let total = months.len();
        if total == 0 {
            return;
        }
        self.download = Some((0, total));
        let tx = self.tx.clone();
        tokio::spawn(async move {
            for (index, month) in months.into_iter().enumerate() {
                let next = month
                    .checked_add_months(chrono::Months::new(1))
                    .unwrap_or(month);
                match records::fetch_range(&client, month, next).await {
                    Ok(records) => {
                        let msg = Msg::CopyMonth {
                            month,
                            records,
                            index,
                            total,
                        };
                        if tx.send(msg).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = tx.send(Msg::CopyFailed(error));
                        return;
                    }
                }
            }
            let _ = tx.send(Msg::CopyDone);
        });
    }

    /// `R`: try the connection and the upload now.
    pub(crate) fn sync_now(&mut self) {
        if self.client.is_none() {
            return;
        }
        if self.needs_login {
            self.status = Some("Session expired, press L to log in".into());
        } else if !self.online {
            self.status = Some("Trying to reach the server…".into());
            self.probe();
        } else if self.outbox.upload_order().is_empty() {
            self.status = Some("Nothing to upload".into());
        } else {
            self.start_sync();
        }
    }

    /// `L`: log in again when the session expired.
    pub(crate) fn relogin(&mut self) {
        if self.needs_login {
            self.log_in();
        } else {
            self.status = Some("Already logged in".into());
        }
    }
}
