use chrono::NaiveDate;
use ratatui::crossterm::event::KeyEvent;
use rolx_core::api::activity::Activity;
use rolx_core::api::balance::Balance;
use rolx_core::api::records::Record;
use rolx_core::api::user_report::UserMonthReport;
use rolx_core::api::year_info::YearInfo;
use rolx_core::client::{RolxClient, SessionStatus};
use std::sync::Arc;

use super::offline::{Load, SyncOutcome};
use super::stamp::StampPart;

pub enum Msg {
    Key(KeyEvent),
    Redraw,
    NeedsLogin,
    LoggedIn,
    LoginFailed(String),
    Connected(Arc<RolxClient>, SessionStatus),
    Header {
        balance: Balance,
        edit_lock: NaiveDate,
    },
    Day {
        day: NaiveDate,
        record: Option<Record>,
        suitable: Vec<Activity>,
    },
    Week {
        monday: NaiveDate,
        records: Vec<Record>,
        suitable: Vec<Activity>,
    },
    Activities {
        all: Vec<Activity>,
        favourites: Vec<Activity>,
    },
    Year(i32, YearInfo),
    Report(String, UserMonthReport),
    Charts {
        /// First day of the month the records belong to.
        month: NaiveDate,
        records: Vec<Record>,
    },
    Saved,
    /// Days the server now holds as these records, to keep in the local copy.
    Learned(Vec<Record>),
    /// The records of the days a stamp covers, loaded to turn the stamp into entries.
    StampDays {
        parts: Vec<StampPart>,
        records: Vec<Record>,
    },
    /// A stamp was booked as this many entries.
    StampBooked(usize),
    FavouritesChanged,
    Error(String),
    /// The server refused a save; the shown day goes back to what the server holds.
    SaveFailed(String),
    /// Loading this from the server failed.
    LoadFailed(Load, anyhow::Error),
    /// Serve this from the local copy.
    FromCopy(Load),
    /// Saving could not reach the server; keep the days as pending changes.
    SaveOffline {
        records: Vec<Record>,
        /// Each day as it was before the change, `None` when the local copy lacks it.
        bases: Vec<Option<Record>>,
        stamp: bool,
        error: anyhow::Error,
    },
    /// The server answered a probe while the app thought it was offline.
    Online,
    ProbeFailed(anyhow::Error),
    /// Time to retry the connection or the upload.
    SyncTick,
    /// Time to refresh the local copy from the server.
    RefreshTick,
    /// One month fetched for the local copy; `index` of `total`, from 0.
    CopyMonth {
        month: NaiveDate,
        records: Vec<Record>,
        index: usize,
        total: usize,
    },
    CopyDone,
    CopyFailed(anyhow::Error),
    SyncDone(Vec<(NaiveDate, SyncOutcome)>),
}

pub enum Screen {
    Connecting,
    Login { busy: bool, error: Option<String> },
    Main,
}
