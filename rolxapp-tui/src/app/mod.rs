pub(crate) mod activities;
mod day;
mod keys;
mod msg;
mod overview;
mod scroll;
mod tasks;

pub use msg::{Msg, Screen};
pub use scroll::Tab;

use crate::edit::EditForm;
use chrono::{Datelike, Local, NaiveDate};
use rolx_core::api::activity::Activity;
use rolx_core::api::balance::Balance;
use rolx_core::api::records::Record;
use rolx_core::api::user_report::UserMonthReport;
use rolx_core::api::year_info::YearInfo;
use rolx_core::client::RolxClient;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct App {
    tx: UnboundedSender<Msg>,
    client: Option<Arc<RolxClient>>,
    pub quit: bool,
    pub screen: Screen,
    pub tab: Tab,
    pub status: Option<String>,
    pub user: String,
    pub balance: Option<Balance>,
    pub edit_lock: Option<NaiveDate>,
    pub day: NaiveDate,
    pub record: Option<Record>,
    pub suitable: Vec<Activity>,
    pub sel_entry: usize,
    pub edit: Option<EditForm>,
    pub activities: Vec<Activity>,
    pub favourites: Vec<i32>,
    pub sel_activity: usize,
    /// Filter applied to the Activities tab.
    pub act_search: String,
    /// Whether keys currently type into `act_search`.
    pub act_searching: bool,
    pub year: i32,
    pub year_info: Option<YearInfo>,
    pub month: NaiveDate,
    pub report: Option<UserMonthReport>,
    /// First keys of a multi-key command such as `gg` or `dd`.
    pub pending: String,
    pub viewport_height: u16,
    pub year_scroll: usize,
    pub report_scroll: usize,
}

impl App {
    pub fn new(tx: UnboundedSender<Msg>) -> Self {
        let today = Local::now().date_naive();
        Self {
            tx,
            client: None,
            quit: false,
            screen: Screen::Connecting,
            tab: Tab::Day,
            status: None,
            user: String::new(),
            balance: None,
            edit_lock: None,
            day: today,
            record: None,
            suitable: Vec::new(),
            sel_entry: 0,
            edit: None,
            activities: Vec::new(),
            favourites: Vec::new(),
            sel_activity: 0,
            act_search: String::new(),
            act_searching: false,
            year: today.year(),
            year_info: None,
            month: today.with_day(1).unwrap_or(today),
            report: None,
            pending: String::new(),
            viewport_height: 24,
            year_scroll: 0,
            report_scroll: 0,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.edit_lock.is_some_and(|lock| self.day < lock)
    }

    pub fn month_key(&self) -> String {
        self.month.format("%Y-%m").to_string()
    }

    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Key(key) => self.on_key(key),
            Msg::Redraw => {}
            Msg::NeedsLogin => {
                self.screen = Screen::Login {
                    busy: false,
                    error: None,
                }
            }
            Msg::LoggedIn => {
                self.screen = Screen::Connecting;
                self.connect();
            }
            Msg::LoginFailed(error) => {
                self.screen = Screen::Login {
                    busy: false,
                    error: Some(error),
                };
            }
            Msg::Connected(client) => {
                self.user = format!(
                    "{} {}",
                    client.session.user_first_name, client.session.user_last_name
                );
                self.client = Some(client);
                self.screen = Screen::Main;
                self.load_header();
                self.load_day();
                self.load_activities();
                self.load_year();
                self.load_report();
            }
            Msg::Header { balance, edit_lock } => {
                self.balance = Some(balance);
                self.edit_lock = Some(edit_lock);
            }
            Msg::Day {
                day,
                record,
                suitable,
            } if day == self.day => {
                let count = record.as_ref().map_or(0, |r| r.entries.len());
                self.sel_entry = self.sel_entry.min(count.saturating_sub(1));
                self.record = record;
                self.suitable = suitable;
                self.sort_suitable();
            }
            Msg::Day { .. } => {}
            Msg::Activities {
                mut all,
                favourites,
            } => {
                self.favourites = favourites.iter().map(|a| a.id).collect();
                all.sort_by_key(|a| !self.favourites.contains(&a.id));
                self.sel_activity = self.sel_activity.min(all.len().saturating_sub(1));
                self.activities = all;
                self.sort_suitable();
            }
            Msg::Year(year, info) if year == self.year => self.year_info = Some(info),
            Msg::Year(..) => {}
            Msg::Report(month, report) if month == self.month_key() => self.report = Some(report),
            Msg::Report(..) => {}
            Msg::Saved => {
                self.status = Some("Saved".into());
                self.load_day();
                self.load_header();
            }
            Msg::FavouritesChanged => self.load_activities(),
            Msg::Error(error) => self.status = Some(error),
        }
    }
}
