//! Drives the fake server with rolx-core's own client code, so it stays compatible with what the apps send and parse.

use chrono::{Datelike, Duration, Local, NaiveDate};
use rolx_core::client::RolxClient;
use rolx_core::dayplan::DayEntry;
use rolx_core::session::Session;
use rolx_core::{activities, balance, dayedit, records, week};
use serde_json::{Value, json};

use super::*;

/// Starts a fake server on a free port, seeded for `today`.
async fn start(today: NaiveDate) -> RolxClient {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_url = format!("http://{}", listener.local_addr().unwrap());
    let state = Arc::new(Mutex::new(seed::state(today)));
    tokio::spawn(serve(listener, state));
    let session: Session = serde_json::from_str(&fake_session::session_json()).unwrap();
    RolxClient {
        http: rolx_core::http::client_builder().build().unwrap(),
        server_url,
        session,
    }
}

/// A Friday, so the seeded week has Monday to Thursday plus today's morning.
fn friday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

fn entry(activity_id: i32, begin: i32, duration: i64) -> DayEntry {
    DayEntry {
        activity_id,
        full_activity_number: String::new(),
        begin: Some(begin),
        duration,
        pause: None,
        comment: None,
    }
}

#[tokio::test]
async fn week_has_entries_until_today_and_no_target_on_weekends() {
    let client = start(friday()).await;
    let week = week::fetch_week(&client, friday()).await.unwrap();
    assert_eq!(week.days.len(), 7);
    for day in &week.days {
        let weekend = day.date.weekday().num_days_from_monday() >= 5;
        assert_eq!(day.nominal_secs, if weekend { 0 } else { 30_240 });
        assert_eq!(day.entries.is_empty(), weekend, "{}", day.date);
    }
    let today = &week.days[4];
    assert_eq!(today.entries.len(), 1, "only the morning of today");
    assert_eq!(today.entries[0].full_activity_number, "#1001.001.01");
}

#[tokio::test]
async fn activities_list_favourites_and_the_closed_one_only_as_recently_booked() {
    let client = start(friday()).await;
    let dir = std::env::temp_dir().join(format!("rolx-fake-server-test-{}", std::process::id()));
    let cache = dir.join("activities.json");

    let all = activities::refresh_to(&client, friday(), &cache)
        .await
        .unwrap();
    assert_eq!(all.len(), 12, "the closed one comes back via `suitable`");
    let mut favourites: Vec<i32> = all.iter().filter(|a| a.favourite).map(|a| a.id).collect();
    favourites.sort();
    assert_eq!(favourites, [1, 5, 10]);
    let closed = all.iter().find(|a| a.id == 9).unwrap();
    assert_eq!(closed.extra["endDate"], "2026-10-23");
    assert_eq!(
        closed.full_name,
        "Alpenbahn AG - Ticketing App - Mobile - Beta Testing (#3107.001.03)"
    );

    let listed: Vec<Value> = client
        .get_json("api/v1/Activity?unlessEndedBeforeDate=2026-10-30")
        .await
        .unwrap();
    assert_eq!(listed.len(), 11);

    let implementation = all.iter().find(|a| a.id == 5).unwrap();
    activities::set_favourite_in(&client, implementation, false, &cache)
        .await
        .unwrap();
    let training = all.iter().find(|a| a.id == 11).unwrap();
    activities::set_favourite_in(&client, training, true, &cache)
        .await
        .unwrap();
    let now: Vec<Value> = client.get_json("api/v1/activity/favourite").await.unwrap();
    let ids: Vec<i64> = now.iter().map(|a| a["id"].as_i64().unwrap()).collect();
    assert_eq!(ids, [1, 10, 11]);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn saving_a_day_replaces_it_and_undo_restores_it() {
    let client = start(friday()).await;
    let day = friday();
    let undo =
        std::env::temp_dir().join(format!("rolx-fake-server-undo-{}.json", std::process::id()));

    let base = dayedit::load_day(&client, day).await.unwrap();
    let mut new = base.clone();
    new.push(DayEntry {
        comment: Some("Nachmittag".into()),
        pause: Some(900),
        ..entry(7, 13 * 3600, 3 * 3600)
    });
    dayedit::save_day_tracked_at(&client, day, &base, &new, "add", &undo)
        .await
        .unwrap();
    let saved = dayedit::load_day(&client, day).await.unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(
        saved[1].full_activity_number, "#3107.001.01",
        "filled in by the server"
    );
    assert_eq!(saved[1].pause, Some(900));

    dayedit::undo_last_at(&client, &undo).await.unwrap();
    assert_eq!(dayedit::load_day(&client, day).await.unwrap(), base);
    let _ = std::fs::remove_file(undo);
}

#[tokio::test]
async fn invalid_days_are_rejected_like_the_real_server_does() {
    let client = start(friday()).await;
    let day = friday();
    let put = |entries: Vec<Value>, date: NaiveDate| {
        let client = &client;
        async move {
            let mut record = records::fetch_day_record(client, date).await.unwrap();
            record["entries"] = Value::Array(entries);
            records::update_day_record(client, date, &record)
                .await
                .map_err(|e| e.to_string())
        }
    };
    let e = |activity: i32, begin: i64, duration: i64| json!({ "activityId": activity, "begin": begin, "duration": duration });

    let closed = put(vec![e(9, 8 * 3600, 3600)], day).await.unwrap_err();
    assert!(
        closed.contains("activity has already been closed"),
        "{closed}"
    );
    let unknown = put(vec![e(99, 8 * 3600, 3600)], day).await.unwrap_err();
    assert!(unknown.contains("existing activity"), "{unknown}");
    let too_long = put(vec![e(1, 23 * 3600, 2 * 3600)], day).await.unwrap_err();
    assert!(too_long.contains("within 24h"), "{too_long}");
    let overlap = put(vec![e(1, 8 * 3600, 7200), e(2, 9 * 3600, 3600)], day)
        .await
        .unwrap_err();
    assert!(overlap.contains("must not overlap"), "{overlap}");

    let lock = dayedit::fetch_edit_lock(&client).await.unwrap();
    assert_eq!(lock, day - Duration::days(4) - Duration::days(7));
    let locked = put(vec![e(1, 8 * 3600, 3600)], lock - Duration::days(1))
        .await
        .unwrap_err();
    assert!(locked.contains("is locked"), "{locked}");
    assert!(put(vec![e(1, 8 * 3600, 3600)], lock).await.is_ok());

    // The closed activity was still open on the day it was booked.
    let last_friday = day - Duration::days(7);
    assert!(put(vec![e(9, 8 * 3600, 3600)], last_friday).await.is_ok());
}

#[tokio::test]
async fn balance_follows_the_booked_time_and_keeps_the_seeded_vacation() {
    let client = start(friday()).await;
    let day = friday();

    // 12:00 before last week, then 4530 of 5040 nominal minutes booked up to today's morning.
    let b = balance::fetch_balance(&client, day).await.unwrap();
    assert_eq!(b.by_date, day);
    assert_eq!(b.overtime_text(), "+3:30 h");
    assert_eq!(b.vacation_text(), "12.5 days (2 planned)");
    let last_sunday = day - Duration::days(5);
    let earlier = balance::fetch_balance(&client, last_sunday).await.unwrap();
    assert_eq!(earlier.overtime_text(), "+10:57 h");

    let base = dayedit::load_day(&client, day).await.unwrap();
    let mut new = base.clone();
    new.push(entry(7, 13 * 3600, 3 * 3600));
    let undo = std::env::temp_dir().join(format!(
        "rolx-fake-server-balance-{}.json",
        std::process::id()
    ));
    dayedit::save_day_tracked_at(&client, day, &base, &new, "add", &undo)
        .await
        .unwrap();
    let _ = std::fs::remove_file(undo);
    let after = balance::fetch_balance(&client, day).await.unwrap();
    assert_eq!(after.overtime_text(), "+6:30 h");

    let invalid = client.get_json::<Value>("api/v1/Balance/not-a-date").await;
    assert!(invalid.unwrap_err().to_string().contains("404"));
}

#[tokio::test]
async fn requests_without_a_token_are_unauthorized_and_extend_gives_a_new_one() {
    let client = start(Local::now().date_naive()).await;
    let url = format!("{}/api/v1/edit-lock", client.server_url);
    let status = client.http.get(&url).send().await.unwrap().status();
    assert_eq!(status, 401);

    let approval: Value = client.get_json("api/v1/SignIn/extend").await.unwrap();
    assert_eq!(approval["user"]["id"], fake_session::USER_ID);
    assert!(
        approval["bearerToken"]
            .as_str()
            .unwrap()
            .starts_with("fake-token-")
    );
    let expires: chrono::DateTime<Utc> =
        serde_json::from_value(approval["expires"].clone()).unwrap();
    assert!(expires > Utc::now() + Duration::hours(23));
}
