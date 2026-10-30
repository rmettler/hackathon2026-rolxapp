use chrono::NaiveDate;
use ratatui::crossterm::event::KeyEvent;
use rolx_core::api::activity::Activity;
use rolx_core::api::balance::Balance;
use rolx_core::api::records::Record;
use rolx_core::api::user_report::UserMonthReport;
use rolx_core::api::year_info::YearInfo;
use rolx_core::client::RolxClient;
use std::sync::Arc;

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
    Saved,
    FavouritesChanged,
    Error(String),
}

pub enum Screen {
    Connecting,
    Login { busy: bool, error: Option<String> },
    Main,
}
