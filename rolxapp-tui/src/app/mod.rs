mod absence;
pub(crate) mod activities;
mod chart;
mod day;
mod keys;
mod msg;
mod offline;
mod overview;
mod resolve;
mod scroll;
mod stamp;
mod tasks;
mod week;
mod week_grid;

pub use absence::{Absence, absence_of, leave_time};
pub use chart::{ChartMeasure, ChartView, MonthChart, slice_at};
pub use msg::{Msg, Screen};
pub use offline::Load;
#[cfg(test)]
pub use offline::SyncOutcome;
pub use resolve::{ConflictDialog, OutboxPopup};
pub use scroll::Tab;
pub use stamp::{StampPart, StampSession};
pub use week::DayView;

use crate::copy::LocalCopy;
use crate::edit::EditForm;
use crate::outbox::Outbox;
use crate::stamp_store::StampStore;
use crate::store::LocalStore;
use chrono::{DateTime, Datelike, Local, NaiveDate};
use rolx_core::api::activity::Activity;
use rolx_core::api::balance::Balance;
use rolx_core::api::records::Record;
use rolx_core::api::user_report::UserMonthReport;
use rolx_core::api::year_info::YearInfo;
use rolx_core::client::{RolxClient, SessionStatus};
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
    pub day_view: DayView,
    /// Records of the week of `day`, Monday first, while the week view is active.
    pub week: Vec<Record>,
    /// Selected activity row in the week grid.
    pub sel_row: usize,
    pub edit: Option<EditForm>,
    /// Selected row of the absence popup while it is open.
    pub absence_menu: Option<usize>,
    pub activities: Vec<Activity>,
    pub favourites: Vec<i32>,
    /// The favourites as activities, so they stay listed even when not in `activities`.
    pub favourite_activities: Vec<Activity>,
    pub sel_activity: usize,
    /// Filter applied to the Activities tab.
    pub act_search: String,
    /// Whether keys currently type into `act_search`.
    pub act_searching: bool,
    pub year: i32,
    pub year_info: Option<YearInfo>,
    pub month: NaiveDate,
    pub report: Option<UserMonthReport>,
    /// First day of the month shown on the Charts tab.
    pub chart_month: NaiveDate,
    /// Records of `chart_month`, `None` while loading.
    pub chart_records: Option<Vec<Record>>,
    /// Selected row of the chart list; row 0 is all activities.
    pub chart_sel: usize,
    /// Activity to reselect once the next month has loaded.
    pub chart_keep: Option<i32>,
    pub chart_measure: ChartMeasure,
    pub chart_view: ChartView,
    /// Start of the running stamp, if any.
    pub stamp: Option<DateTime<Local>>,
    /// Where the running stamp is kept; `None` keeps it in memory only.
    pub stamp_store: Option<StampStore>,
    /// The days of the stamp being stamped out while its form is open.
    pub stamp_session: Option<StampSession>,
    /// Whether the discard-the-stamp question is waiting for an answer.
    pub discard_prompt: bool,
    /// First keys of a multi-key command such as `gg` or `dd`.
    pub pending: String,
    pub viewport_height: u16,
    pub year_scroll: usize,
    pub report_scroll: usize,
    /// Where the local copy and the outbox are kept; `None` keeps them in memory only.
    pub store: Option<LocalStore>,
    pub copy: LocalCopy,
    pub outbox: Outbox,
    /// Whether the last request to the server got through.
    pub online: bool,
    /// Whether the saved login is no longer accepted, so nothing can be uploaded.
    pub needs_login: bool,
    /// Whether the outbox is being uploaded.
    pub syncing: bool,
    /// The list of pending changes while it is open.
    pub outbox_popup: Option<OutboxPopup>,
    /// The question for a conflicting day while it is open.
    pub conflict: Option<ConflictDialog>,
    /// When the shown balance was fetched, while it comes from the local copy.
    pub balance_at: Option<DateTime<Local>>,
    pub year_at: Option<DateTime<Local>>,
    pub report_at: Option<DateTime<Local>>,
    /// Months fetched so far and in total while the local copy is being refreshed.
    pub download: Option<(usize, usize)>,
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
            day_view: DayView::Week,
            week: Vec::new(),
            sel_row: 0,
            edit: None,
            absence_menu: None,
            activities: Vec::new(),
            favourites: Vec::new(),
            favourite_activities: Vec::new(),
            sel_activity: 0,
            act_search: String::new(),
            act_searching: false,
            year: today.year(),
            year_info: None,
            month: today.with_day(1).unwrap_or(today),
            report: None,
            chart_month: today.with_day(1).unwrap_or(today),
            chart_records: None,
            chart_sel: 0,
            chart_keep: None,
            chart_measure: ChartMeasure::RunningTotal,
            chart_view: ChartView::Line,
            stamp: None,
            stamp_store: None,
            stamp_session: None,
            discard_prompt: false,
            pending: String::new(),
            viewport_height: 24,
            year_scroll: 0,
            report_scroll: 0,
            store: None,
            copy: LocalCopy::default(),
            outbox: Outbox::default(),
            online: true,
            needs_login: false,
            syncing: false,
            outbox_popup: None,
            conflict: None,
            balance_at: None,
            year_at: None,
            report_at: None,
            download: None,
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
                if self.client.is_some() {
                    // Back to the local copy, which still works.
                    self.screen = Screen::Main;
                    self.status = Some(format!("Login failed: {error}"));
                } else {
                    self.screen = Screen::Login {
                        busy: false,
                        error: Some(error),
                    };
                }
            }
            Msg::Connected(client, status) => {
                self.user = format!(
                    "{} {}",
                    client.session.user_first_name, client.session.user_last_name
                );
                self.client = Some(client);
                self.online = true;
                self.needs_login = false;
                if status == SessionStatus::Expired {
                    if self.copy.is_empty() {
                        self.client = None;
                        self.update(Msg::NeedsLogin);
                        return;
                    }
                    self.needs_login = true;
                    self.status = Some("Session expired, press L to log in".into());
                }
                self.screen = Screen::Main;
                self.load_header();
                self.load_view();
                self.load_activities();
                self.load_year();
                self.load_report();
                self.start_sync();
                self.start_download();
            }
            Msg::Header { balance, edit_lock } => {
                self.learn_header(&balance, edit_lock);
                self.balance = Some(balance);
                self.balance_at = None;
                self.edit_lock = Some(edit_lock);
            }
            Msg::Day {
                day,
                record,
                suitable,
            } => {
                if let Some(record) = &record {
                    self.learn_days(std::slice::from_ref(record));
                }
                self.apply_day(day, record, suitable);
            }
            Msg::Week {
                monday,
                records,
                suitable,
            } => {
                self.learn_days(&records);
                self.apply_week(monday, records, suitable);
            }
            Msg::Activities { all, favourites } => {
                self.learn_activities(&all, &favourites);
                self.apply_activities(all, favourites);
            }
            Msg::Year(year, info) => {
                self.learn_year(year, &info);
                self.apply_year(year, info);
                self.year_at = None;
            }
            Msg::Report(month, report) => {
                self.learn_report(&month, &report);
                self.apply_report(month, report);
                self.report_at = None;
            }
            Msg::Charts { month, records } => {
                self.learn_month(month, &records);
                self.apply_charts(month, records);
            }
            Msg::StampDays { parts, records } => {
                self.learn_days(&records);
                let records = records
                    .into_iter()
                    .map(|r| self.outbox.overlay(r))
                    .collect();
                self.open_stamp_form(parts, records);
            }
            Msg::StampBooked(count) => {
                self.status = Some(format!(
                    "Booked {count} {}",
                    if count == 1 { "entry" } else { "entries" }
                ));
                self.clear_stamp();
                self.load_view();
                self.load_header();
            }
            Msg::Saved => {
                self.status = Some("Saved".into());
                self.load_view();
                self.load_header();
            }
            Msg::Learned(records) => self.learn_days(&records),
            Msg::FavouritesChanged => self.load_activities(),
            Msg::Error(error) => self.status = Some(error),
            Msg::SaveFailed(error) => {
                self.status = Some(error);
                self.load_view();
            }
            Msg::LoadFailed(load, error) => self.on_load_failed(load, error),
            Msg::FromCopy(load) => self.serve_from_copy(load),
            Msg::SaveOffline {
                records,
                bases,
                stamp,
                error,
            } => {
                self.note_failure(&error);
                self.queue_days(records, bases, stamp);
            }
            Msg::Online => self.on_online(),
            Msg::ProbeFailed(error) => {
                self.note_failure(&error);
            }
            Msg::SyncTick => self.on_sync_tick(),
            Msg::RefreshTick => self.start_download(),
            Msg::CopyMonth {
                month,
                records,
                index,
                total,
            } => {
                self.learn_month(month, &records);
                self.download = Some((index + 1, total));
            }
            Msg::CopyDone => self.download = None,
            Msg::CopyFailed(error) => {
                self.download = None;
                if !self.note_failure(&error) {
                    self.status = Some(format!("Refreshing the local copy failed: {error:#}"));
                }
            }
            Msg::SyncDone(outcomes) => self.finish_sync(outcomes),
        }
    }
}
