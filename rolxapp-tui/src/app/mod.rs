mod absence;
pub(crate) mod activities;
mod chart;
mod day;
mod keys;
mod msg;
mod overview;
mod scroll;
mod stamp;
mod tasks;
mod week;
mod week_grid;

pub use absence::{Absence, absence_of, leave_time};
pub use chart::ChartMeasure;
pub use msg::{Msg, Screen};
pub use scroll::Tab;
pub use stamp::{StampPart, StampSession};
pub use week::DayView;

use crate::edit::EditForm;
use crate::stamp_store::StampStore;
use crate::time::week_start;
use chrono::{DateTime, Datelike, Local, NaiveDate};
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
            day_view: DayView::Day,
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
            stamp: None,
            stamp_store: None,
            stamp_session: None,
            discard_prompt: false,
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
            } if day == self.day && self.day_view == DayView::Day => {
                self.record = record;
                self.suitable = suitable;
                self.sort_suitable();
                self.sel_entry = self.sel_entry.min(self.day_row_count().saturating_sub(1));
            }
            Msg::Day { .. } => {}
            Msg::Week {
                monday,
                records,
                suitable,
            } if self.day_view == DayView::Week && monday == week_start(self.day) => {
                self.week = records;
                self.suitable = suitable;
                self.sort_suitable();
                self.sync_week_record();
                let rows = self.week_grid().rows.len();
                self.sel_row = self.sel_row.min(rows.saturating_sub(1));
            }
            Msg::Week { .. } => {}
            Msg::Activities {
                mut all,
                favourites,
            } => {
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
            Msg::Year(year, info) if year == self.year => self.year_info = Some(info),
            Msg::Year(..) => {}
            Msg::Report(month, report) if month == self.month_key() => self.report = Some(report),
            Msg::Report(..) => {}
            Msg::Charts { month, records } if month == self.chart_month => {
                self.chart_records = Some(records);
                let chart = self.month_chart();
                let len = chart.as_ref().map_or(0, |c| c.series.len());
                self.chart_sel = match self.chart_keep.take() {
                    Some(id) => chart
                        .and_then(|c| c.series.iter().position(|s| s.activity_id == Some(id)))
                        .unwrap_or(0),
                    None => self.chart_sel.min(len.saturating_sub(1)),
                };
            }
            Msg::Charts { .. } => {}
            Msg::StampDays { parts, records } => self.open_stamp_form(parts, records),
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
            Msg::FavouritesChanged => self.load_activities(),
            Msg::Error(error) => self.status = Some(error),
        }
    }
}
