use chrono::NaiveDate;
use ratatui::crossterm::event::KeyEvent;
use rolx_core::api::activity::Activity;
use rolx_core::api::balance::Balance;
use rolx_core::api::records::Record;
use rolx_core::api::user_report::UserMonthReport;
use rolx_core::api::year_info::YearInfo;
use rolx_core::client::RolxClient;
use std::sync::Arc;

use super::stamp::StampPart;

pub enum Msg {
    Key(KeyEvent),
    Redraw,
    NeedsLogin,
    LoggedIn,
    LoginFailed(String),
    Connected(Arc<RolxClient>),
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
    /// The records of the days a stamp covers, loaded to turn the stamp into entries.
    StampDays {
        parts: Vec<StampPart>,
        records: Vec<Record>,
    },
    /// A stamp was booked as this many entries.
    StampBooked(usize),
    FavouritesChanged,
    Error(String),
}

pub enum Screen {
    Connecting,
    Login { busy: bool, error: Option<String> },
    Main,
}
