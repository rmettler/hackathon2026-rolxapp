use super::{App, Msg, Screen};
use crate::time::week_start;
use chrono::{Datelike, Duration, Local};
use rolx_core::api::activity;
use rolx_core::api::balance;
use rolx_core::api::edit_lock;
use rolx_core::api::records::{self, Record};
use rolx_core::api::user_report;
use rolx_core::api::year_info;
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::login::{self, AuthFlow};
use rolx_core::session::{LoadError, Session};
use std::future::Future;
use std::sync::Arc;

impl App {
    pub fn connect(&self) {
        self.spawn(async {
            match Session::load() {
                Err(LoadError::NoSession) => return Ok(Msg::NeedsLogin),
                Ok(session) if session.is_expired() => return Ok(Msg::NeedsLogin),
                _ => {}
            }
            let config = AppConfig::load()?;
            Ok(Msg::Connected(Arc::new(RolxClient::new(&config).await?)))
        });
    }

    pub(crate) fn spawn(&self, task: impl Future<Output = anyhow::Result<Msg>> + Send + 'static) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = task.await.unwrap_or_else(|e| Msg::Error(format!("{e:#}")));
            let _ = tx.send(msg);
        });
    }

    pub(crate) fn spawn_with_client<F, Fut>(&self, f: F)
    where
        F: FnOnce(Arc<RolxClient>) -> Fut,
        Fut: Future<Output = anyhow::Result<Msg>> + Send + 'static,
    {
        if let Some(client) = &self.client {
            self.spawn(f(client.clone()));
        }
    }

    pub(crate) fn load_header(&self) {
        self.spawn_with_client(|c| async move {
            let today = Local::now().date_naive();
            let (balance, edit_lock) =
                tokio::try_join!(balance::fetch(&c, today), edit_lock::fetch(&c))?;
            Ok(Msg::Header { balance, edit_lock })
        });
    }

    pub(crate) fn load_day(&self) {
        let day = self.day;
        self.spawn_with_client(move |c| async move {
            let (record, suitable) = tokio::try_join!(
                records::fetch_record(&c, day),
                activity::fetch_suitable(&c, day)
            )?;
            Ok(Msg::Day {
                day,
                record,
                suitable,
            })
        });
    }

    pub(crate) fn load_week(&self) {
        let day = self.day;
        let monday = week_start(day);
        self.spawn_with_client(move |c| async move {
            let (records, suitable) = tokio::try_join!(
                records::fetch_range(&c, monday, monday + Duration::days(7)),
                activity::fetch_suitable(&c, day)
            )?;
            Ok(Msg::Week {
                monday,
                records,
                suitable,
            })
        });
    }

    pub(crate) fn load_activities(&self) {
        self.spawn_with_client(|c| async move {
            let today = Local::now().date_naive();
            let (all, favourites) = tokio::try_join!(
                activity::fetch_all(&c, today),
                activity::fetch_favourites(&c)
            )?;
            Ok(Msg::Activities { all, favourites })
        });
    }

    pub(crate) fn load_year(&self) {
        let year = self.year;
        self.spawn_with_client(move |c| async move {
            Ok(Msg::Year(year, year_info::fetch(&c, year).await?))
        });
    }

    pub(crate) fn load_report(&self) {
        let month = self.month_key();
        self.spawn_with_client(move |c| async move {
            let report = user_report::fetch(&c, &month).await?;
            Ok(Msg::Report(month, report))
        });
    }

    pub(crate) fn load_charts(&self) {
        let first = self.chart_month;
        let next = (first + Duration::days(32)).with_day(1).unwrap_or(first);
        self.spawn_with_client(move |c| async move {
            let records = records::fetch_range(&c, first, next).await?;
            Ok(Msg::Charts {
                month: first,
                records,
            })
        });
    }

    pub(crate) fn put(&self, record: Record) {
        self.spawn_with_client(move |c| async move {
            records::put_record(&c, &record).await?;
            Ok(Msg::Saved)
        });
    }

    pub(crate) fn log_in(&mut self) {
        self.screen = Screen::Login {
            busy: true,
            error: None,
        };
        let tx = self.tx.clone();
        // The login future is not `Send`, so drive it on a blocking thread.
        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let result = handle.block_on(async {
                let config = AppConfig::load()?;
                login::run(&config, AuthFlow::NativeApp).await?;
                anyhow::Ok(())
            });
            let _ = tx.send(match result {
                Ok(()) => Msg::LoggedIn,
                Err(e) => Msg::LoginFailed(format!("{e:#}")),
            });
        });
    }
}
