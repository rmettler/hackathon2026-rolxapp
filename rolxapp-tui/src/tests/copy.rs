use chrono::{Duration, Local, NaiveDate, TimeZone};
use rolx_core::api::balance::Balance;
use rolx_core::api::records::Record;
use rolx_core::api::year_info::{Holiday, YearInfo};
use rolx_core::client::ApiError;

use crate::app::{Load, Msg, Tab};
use crate::copy::LocalCopy;
use crate::test_support::*;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn balance() -> Balance {
    Balance {
        by_date: "2026-10-30".into(),
        overtime: Duration::minutes(130),
        vacation_available: Duration::hours(80),
        vacation_available_days: Some(10.0),
        vacation_planned: Duration::zero(),
        vacation_planned_days: None,
    }
}

fn record(day: &str) -> Record {
    let mut record = main_app().record.unwrap();
    record.date = day.into();
    record
}

#[test]
fn fresh_numbers_have_no_age_and_cached_ones_say_when_they_were_fetched() {
    let mut app = main_app();
    app.update(Msg::Header {
        balance: balance(),
        edit_lock: date(2026, 10, 1),
    });
    assert!(app.balance_at.is_none());
    assert!(!render(&app).contains("as of"));

    app.update(Msg::FromCopy(Load::Header));

    assert!(app.balance_at.is_some());
    assert!(render(&app).contains("Overtime 2:10"));
    assert!(render(&app).contains("(as of "));

    app.update(Msg::Header {
        balance: balance(),
        edit_lock: date(2026, 10, 1),
    });
    assert!(
        app.balance_at.is_none(),
        "fresh again once the server answers"
    );
}

#[test]
fn the_year_from_the_copy_says_when_it_was_fetched() {
    let mut app = main_app();
    app.year = 2026;
    app.tab = Tab::Year;
    let info = YearInfo {
        holidays: vec![Holiday {
            name: "Neujahr".into(),
            date: "2026-01-01".into(),
        }],
        monthly_work_times: Vec::new(),
    };
    app.update(Msg::Year(2026, info));
    assert!(!render(&app).contains("as of"));

    app.update(Msg::FromCopy(Load::Year(2026)));

    assert!(render(&app).contains("Neujahr"));
    assert!(render(&app).contains("(as of "));
}

#[test]
fn nothing_cached_means_nothing_is_shown_instead_of_guessing() {
    let mut app = main_app();

    app.update(Msg::FromCopy(Load::Header));
    app.update(Msg::FromCopy(Load::Year(2026)));

    assert!(app.balance.is_none());
    assert!(app.year_info.is_none());
}

#[test]
fn the_first_start_offline_says_there_is_no_copy_yet() {
    let mut app = main_app();
    app.record = None;

    app.update(Msg::FromCopy(Load::Day(date(2026, 10, 30))));

    assert!(app.status.as_deref().unwrap().contains("No local copy yet"));
}

#[test]
fn a_fetched_month_enters_the_copy_and_counts_as_downloaded() {
    let mut app = main_app();
    app.download = Some((0, 26));

    app.update(Msg::CopyMonth {
        month: date(2026, 9, 1),
        records: vec![record("2026-09-14")],
        index: 11,
        total: 26,
    });

    assert!(app.copy.day(date(2026, 9, 14)).is_some());
    assert!(app.copy.month_fetched(date(2026, 9, 1)).is_some());
    assert_eq!(app.download, Some((12, 26)));
    assert!(render(&app).contains("Downloading 12/26"));

    app.update(Msg::CopyDone);
    assert!(app.download.is_none());
    assert!(!render(&app).contains("Downloading"));
}

#[test]
fn a_short_refresh_is_not_announced() {
    let mut app = main_app();

    app.update(Msg::CopyMonth {
        month: date(2026, 10, 1),
        records: Vec::new(),
        index: 0,
        total: 3,
    });

    assert!(!render(&app).contains("Downloading"));
}

#[test]
fn a_lost_connection_stops_the_download() {
    let mut app = main_app();
    app.download = Some((3, 26));

    app.update(Msg::CopyFailed(ApiError::Network("gone".into()).into()));

    assert!(app.download.is_none());
    assert!(!app.online);
}

#[test]
fn nothing_is_downloaded_without_a_connection() {
    let mut app = main_app();

    app.start_download();

    assert!(app.download.is_none());
}

#[test]
fn months_outside_the_window_are_dropped_from_disk_on_start() {
    let store = temp_local_store();
    let old = date(2020, 1, 1);
    let mut copy = LocalCopy::default();
    copy.put_month(
        old,
        &[record("2020-01-15")],
        Local.with_ymd_and_hms(2020, 2, 1, 0, 0, 0).unwrap(),
    );
    copy.save_months(&store, [old]).unwrap();
    let mut app = main_app();

    app.install_store(store.clone());

    assert!(app.copy.day(date(2020, 1, 15)).is_none());
    assert!(store.list("records").unwrap().is_empty());
}
