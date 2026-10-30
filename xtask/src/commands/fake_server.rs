//! `cargo xtask fake-server`: an in-memory stand-in for the RolX server, to try the CLI and the apps end to end without
//! touching real data.
//!
//! Implements the endpoints rolx-core calls, with the shapes and the main validation rules of the real server
//! (`rolx-server/source/RolXServer/RolXServer/**/WebApi`). Any bearer token and any user id are accepted; all users
//! share the same data. Everything is lost when the server stops.

mod seed;

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::Response;
use chrono::{Datelike, Duration, Local, NaiveDate, SecondsFormat, Utc, Weekday};
use serde::Deserialize;
use serde_json::{Value, json};

use super::fake_session;

pub const DEFAULT_PORT: u16 = 5080;
const SECONDS_PER_DAY: i64 = 24 * 3600;
/// 8.4 hours, the nominal time of a full-time workday (100% pensum).
const NOMINAL_WORK_TIME: i64 = 30_240;
/// Longest range served at once, so a typo doesn't build a huge response.
const MAX_RANGE_DAYS: i64 = 1000;

#[derive(clap::Args)]
pub struct FakeServerArgs {
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,
    /// Address to listen on. The emulator reaches 127.0.0.1 via `adb reverse` or 10.0.2.2; use 0.0.0.0 for phones
    /// in the same network.
    #[arg(long, default_value = "127.0.0.1")]
    bind: IpAddr,
}

pub fn run(args: &FakeServerArgs) -> Result<String, String> {
    let today = Local::now().date_naive();
    let state = Arc::new(Mutex::new(seed::state(today)));
    let addr = SocketAddr::new(args.bind, args.port);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("cannot start the async runtime: {e}"))?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("cannot listen on {addr}: {e}"))?;
        print_banner(addr, &state.lock().expect("not poisoned"));
        serve(listener, state)
            .await
            .map_err(|e| format!("server failed: {e}"))
    })?;
    Ok("fake server stopped".into())
}

async fn serve(listener: tokio::net::TcpListener, state: Shared) -> std::io::Result<()> {
    let app = axum::Router::new().fallback(move |method, uri, headers, body| {
        handle(state.clone(), method, uri, headers, body)
    });
    axum::serve(listener, app).await
}

fn print_banner(addr: SocketAddr, state: &State) {
    let first = state.days.keys().next().copied();
    let last = state.days.keys().next_back().copied();
    println!("RolX fake server on http://{addr}  (state: GET http://{addr}/)");
    println!(
        "  {} activities ({} favourites, {} closed), entries {} .. {}, edit lock {}",
        state.activities.len(),
        state.favourites.len(),
        state
            .activities
            .iter()
            .find(|a| a.end.is_some())
            .map_or(String::new(), Activity::full_number),
        first.map_or("-".into(), |d| d.to_string()),
        last.map_or("-".into(), |d| d.to_string()),
        state.edit_lock,
    );
    println!(
        "  CLI:     $env:ROLX_SERVER='http://{addr}'; $env:ROLX_DATA_DIR=<temp dir>; cargo run -- records"
    );
    println!("  Android: cargo xtask android run --fake-server; cargo xtask android push-session");
    println!("  Ctrl-C stops it; everything is kept in memory only.");
}

type Shared = Arc<Mutex<State>>;

async fn handle(
    state: Shared,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let reply = {
        let mut state = state.lock().expect("a handler panicked");
        route(&mut state, &method, &uri, &headers, &body)
    };
    let note = match &reply.body {
        _ if reply.status.is_success() => String::new(),
        ReplyBody::Text(text) => format!("  {text}"),
        ReplyBody::Json(v) => match v.get("errors") {
            Some(errors) => format!("  {errors}"),
            None => format!("  {v}"),
        },
        ReplyBody::Empty => String::new(),
    };
    println!(
        "{} {method} {uri} {}{note}",
        Local::now().format("%H:%M:%S"),
        reply.status.as_u16()
    );
    reply.into_response()
}

/// The answer to a request, before it becomes an HTTP response.
struct Reply {
    status: StatusCode,
    body: ReplyBody,
}

enum ReplyBody {
    Empty,
    Json(Value),
    Text(String),
}

impl Reply {
    fn json(value: Value) -> Self {
        Self {
            status: StatusCode::OK,
            body: ReplyBody::Json(value),
        }
    }

    fn status(status: StatusCode) -> Self {
        Self {
            status,
            body: ReplyBody::Empty,
        }
    }

    /// Like ASP.NET's `BadRequest("...")`.
    fn bad_request(text: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: ReplyBody::Text(text.into()),
        }
    }

    /// The `ValidationProblemDetails` ASP.NET sends when FluentValidation rejects a body.
    fn validation_problem(errors: &[(String, String)]) -> Self {
        let mut by_field: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (field, message) in errors {
            by_field.entry(field).or_default().push(message);
        }
        Self {
            status: StatusCode::BAD_REQUEST,
            body: ReplyBody::Json(json!({
                "type": "https://tools.ietf.org/html/rfc9110#section-15.5.1",
                "title": "One or more validation errors occurred.",
                "status": 400,
                "errors": by_field,
            })),
        }
    }

    fn into_response(self) -> Response {
        let builder = Response::builder().status(self.status);
        let response = match self.body {
            ReplyBody::Empty => builder.body(Body::empty()),
            ReplyBody::Json(v) => builder
                .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
                .body(Body::from(v.to_string())),
            ReplyBody::Text(t) => builder
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(Body::from(t)),
        };
        response.expect("status and headers are valid")
    }
}

fn route(state: &mut State, method: &Method, uri: &Uri, headers: &HeaderMap, body: &[u8]) -> Reply {
    let raw: Vec<&str> = uri.path().split('/').filter(|s| !s.is_empty()).collect();
    // ASP.NET routes are case-insensitive (rolx-core itself writes both `Activity` and `activity`).
    let lower: Vec<String> = raw.iter().map(|s| s.to_ascii_lowercase()).collect();
    let lower: Vec<&str> = lower.iter().map(String::as_str).collect();

    if method == Method::GET && lower.is_empty() {
        return Reply::json(state.overview());
    }
    let ["api", "v1", rest @ ..] = lower.as_slice() else {
        return Reply::status(StatusCode::NOT_FOUND);
    };
    let raw = &raw[2..];

    match (method.as_str(), rest) {
        ("GET", ["signin", "info"]) => {
            return Reply::json(json!({
                "googleClientId": "fake-google-client-id.apps.googleusercontent.com",
                "installationId": "fake-installation",
            }));
        }
        ("POST", ["signin"]) => return sign_in(body),
        _ => {}
    }

    let authorized = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| !token.trim().is_empty());
    if !authorized {
        return Reply::status(StatusCode::UNAUTHORIZED);
    }

    match (method.as_str(), rest) {
        ("GET", ["signin", "extend"]) => Reply::json(approval()),
        ("GET", ["workrecord", _, "range", _]) => state.get_range(raw[1], raw[3]),
        ("PUT", ["workrecord", _, _]) => state.put_record(raw[1], raw[2], body),
        ("GET", ["edit-lock"]) => Reply::json(json!({ "date": iso(state.edit_lock) })),
        ("GET", ["activity"]) => state.all_activities(uri.query()),
        ("GET", ["activity", "suitable", _, _]) => state.suitable_activities(raw[3]),
        ("GET", ["activity", "favourite"]) => state.favourite_activities(),
        ("PUT", ["activity", "favourite"]) => state.set_favourite(body, true),
        ("DELETE", ["activity", "favourite"]) => state.set_favourite(body, false),
        ("GET", ["activity", id]) => match id.parse::<i32>().ok().and_then(|id| state.activity(id))
        {
            Some(a) => Reply::json(state.activity_json(a)),
            None => Reply::status(StatusCode::NOT_FOUND),
        },
        _ => Reply::status(StatusCode::NOT_FOUND),
    }
}

/// `POST SignIn`: the real server checks the Google ID token, this one accepts any.
fn sign_in(body: &[u8]) -> Reply {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SignInData {
        google_id_token: String,
    }
    match serde_json::from_slice::<SignInData>(body) {
        Ok(data) if !data.google_id_token.is_empty() => Reply::json(approval()),
        _ => Reply::validation_problem(&[(
            "GoogleIdToken".into(),
            "The GoogleIdToken field is required.".into(),
        )]),
    }
}

/// The server's `Approval` resource with a fresh token, valid for a day like the real one.
fn approval() -> Value {
    let now = Utc::now();
    json!({
        "user": fake_session::user_json(),
        "bearerToken": format!("fake-token-{}", now.timestamp_millis()),
        "expires": (now + Duration::days(1)).to_rfc3339_opts(SecondsFormat::Millis, true),
    })
}

fn iso(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn parse_iso(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

/// A bookable activity, like the server's domain model (`Projects/Domain/Model/Activity.cs`).
pub struct Activity {
    id: i32,
    number: u32,
    name: String,
    project_number: u32,
    subproject_number: u32,
    customer: String,
    project: String,
    subproject: String,
    billable: bool,
    start: NaiveDate,
    /// Last day it can be booked on (the resource's `endDate`); `None` while open.
    end: Option<NaiveDate>,
    /// Seconds; 0 means no budget.
    budget: i64,
}

impl Activity {
    fn full_number(&self) -> String {
        format!(
            "#{:04}.{:03}.{:02}",
            self.project_number, self.subproject_number, self.number
        )
    }

    fn all_subproject_names(&self) -> String {
        format!("{} - {} - {}", self.customer, self.project, self.subproject)
    }

    /// Why it can't be booked on `date`, as the server words it.
    fn closed_reason(&self, date: NaiveDate) -> Option<&'static str> {
        if self.start > date {
            Some("activity isn't open yet")
        } else if self.end.is_some_and(|end| end < date) {
            Some("activity has already been closed")
        } else {
            None
        }
    }
}

/// An entry as stored, after the server's sanitizing (no empty entries, no zero pause, comment never null).
#[derive(Clone)]
pub struct Entry {
    activity_id: i32,
    duration: i64,
    begin: Option<i64>,
    pause: Option<i64>,
    comment: String,
}

pub struct Day {
    paid_leave_type: Option<i64>,
    paid_leave_reason: Option<String>,
    entries: Vec<Entry>,
}

pub struct State {
    activities: Vec<Activity>,
    favourites: BTreeSet<i32>,
    days: BTreeMap<NaiveDate, Day>,
    /// Days before this one can't be changed.
    edit_lock: NaiveDate,
}

/// The body of `PUT WorkRecord/{userId}/{date}` (`Records/WebApi/Resource/Record.cs`); the rest is ignored like the
/// real server ignores it (day type and nominal time come from its calendar).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordIn {
    date: String,
    user_id: String,
    #[serde(default)]
    paid_leave_type: Option<i64>,
    #[serde(default)]
    paid_leave_reason: Option<String>,
    #[serde(default)]
    entries: Vec<EntryIn>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryIn {
    activity_id: i32,
    duration: i64,
    #[serde(default)]
    begin: Option<i64>,
    #[serde(default)]
    pause: Option<i64>,
    #[serde(default)]
    comment: Option<String>,
}

impl State {
    fn activity(&self, id: i32) -> Option<&Activity> {
        self.activities.iter().find(|a| a.id == id)
    }

    fn activity_json(&self, a: &Activity) -> Value {
        let actual: i64 = self
            .days
            .values()
            .flat_map(|d| &d.entries)
            .filter(|e| e.activity_id == a.id)
            .map(|e| e.duration)
            .sum();
        let budget_fraction = if a.budget > 0 {
            actual as f64 / a.budget as f64
        } else {
            0.0
        };
        let full_number = a.full_number();
        json!({
            "id": a.id,
            "number": a.number,
            "name": a.name,
            "startDate": iso(a.start),
            "endDate": a.end.map(iso),
            "billabilityId": if a.billable { 1 } else { 2 },
            "billabilityName": if a.billable { "Billable" } else { "Non-billable" },
            "isBillable": a.billable,
            "isOverBudget": a.budget > 0 && actual > a.budget,
            "isOverPlanned": false,
            "budget": a.budget,
            "planned": 0,
            "actual": actual,
            "budgetConsumedFraction": budget_fraction,
            "plannedConsumedFraction": 0.0,
            "projectName": a.project,
            "subprojectName": a.subproject,
            "customerName": a.customer,
            "fullName": format!("{} - {} ({full_number})", a.all_subproject_names(), a.name),
            "fullNumber": full_number,
            "allSubprojectNames": a.all_subproject_names(),
        })
    }

    fn entry_json(&self, e: &Entry) -> Value {
        json!({
            "activityId": e.activity_id,
            "fullActivityNumber": self.activity(e.activity_id).map(Activity::full_number).unwrap_or_default(),
            "duration": e.duration,
            "begin": e.begin,
            "pause": e.pause,
            "comment": e.comment,
        })
    }

    /// The server's `Record` resource for `date`; days without anything stored are empty records.
    fn record_json(&self, date: NaiveDate, user_id: &str) -> Value {
        let weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
        let day = self.days.get(&date);
        json!({
            "date": iso(date),
            "userId": user_id,
            "dayType": if weekend { 1 } else { 0 },
            "dayName": "",
            "nominalWorkTime": if weekend { 0 } else { NOMINAL_WORK_TIME },
            "paidLeaveType": day.and_then(|d| d.paid_leave_type),
            "paidLeaveReason": day.and_then(|d| d.paid_leave_reason.clone()),
            "entries": day.map_or(vec![], |d| d.entries.iter().map(|e| self.entry_json(e)).collect()),
        })
    }

    /// `GET WorkRecord/{userId}/range/{begin}..{end}`: one record per day, `end` excluded.
    fn get_range(&self, user_id: &str, range: &str) -> Reply {
        let Some((begin, end)) = range
            .split_once("..")
            .and_then(|(b, e)| Some((parse_iso(b)?, parse_iso(e)?)))
        else {
            return Reply::status(StatusCode::NOT_FOUND);
        };
        let days = (end - begin).num_days();
        if !(0..=MAX_RANGE_DAYS).contains(&days) {
            return Reply::bad_request("invalid range");
        }
        let records: Vec<Value> = (0..days)
            .map(|i| self.record_json(begin + Duration::days(i), user_id))
            .collect();
        Reply::json(Value::Array(records))
    }

    /// `PUT WorkRecord/{userId}/{date}`: replaces the whole day, after the checks of `RecordValidator` and
    /// `RecordEntryValidator` that matter for a single user.
    fn put_record(&mut self, user_id: &str, date: &str, body: &[u8]) -> Reply {
        let Some(day) = parse_iso(date) else {
            return Reply::status(StatusCode::NOT_FOUND);
        };
        let record: RecordIn = match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => {
                return Reply::validation_problem(&[("$".into(), format!("invalid record: {e}"))]);
            }
        };

        let mut errors: Vec<(String, String)> = Vec::new();
        let mut fail = |field: String, message: &str| errors.push((field, message.to_string()));
        if day < self.edit_lock {
            fail(
                "Date".into(),
                &format!("Editing before {} is locked.", iso(self.edit_lock)),
            );
        }
        for (i, e) in record.entries.iter().enumerate() {
            let field = |name: &str| format!("Entries[{i}].{name}");
            if e.activity_id == 0 {
                fail(
                    field("ActivityId"),
                    "'Activity Id' must not be equal to '0'.",
                );
            } else if e.duration != 0 {
                match self.activity(e.activity_id) {
                    None => fail(
                        field("ActivityId"),
                        "activityId must be of an existing activity",
                    ),
                    Some(a) => {
                        if let Some(reason) = a.closed_reason(day) {
                            fail(field("ActivityId"), reason);
                        }
                    }
                }
            }
            if e.duration < 0 {
                fail(
                    field("Duration"),
                    "'Duration' must be greater than or equal to '0'.",
                );
            }
            if e.begin.is_some_and(|b| !(0..=SECONDS_PER_DAY).contains(&b)) {
                fail(field("Begin"), "'Begin' must be between 0 and 86400.");
            }
            if e.pause.is_some_and(|p| !(0..=SECONDS_PER_DAY).contains(&p)) {
                fail(field("Pause"), "'Pause' must be between 0 and 86400.");
            }
            if e.pause.is_some() && e.begin.is_none() {
                fail(field("Pause"), "'Pause' must be empty.");
            }
            if let Some(begin) = e.begin
                && begin + e.duration + e.pause.unwrap_or(0) > SECONDS_PER_DAY
            {
                fail(
                    field("Duration"),
                    "begin + pause + duration must be within 24h",
                );
            }
        }
        // Like the real server, the overlap check ignores the pause.
        let mut spans: Vec<(i64, i64)> = record
            .entries
            .iter()
            .filter_map(|e| e.begin.map(|b| (b, b + e.duration)))
            .collect();
        spans.sort();
        if spans.windows(2).any(|w| w[0].1 > w[1].0) {
            fail("Entries".into(), "Record entries must not overlap.");
        }
        if !errors.is_empty() {
            return Reply::validation_problem(&errors);
        }
        // The controller checks these after validation.
        if record.date != date {
            return Reply::bad_request("non-matching date");
        }
        if !record.user_id.eq_ignore_ascii_case(user_id) {
            return Reply::bad_request("non-matching user id");
        }

        let entries: Vec<Entry> = record
            .entries
            .into_iter()
            .filter(|e| e.duration != 0)
            .map(|e| Entry {
                activity_id: e.activity_id,
                duration: e.duration,
                begin: e.begin,
                pause: e.pause.filter(|&p| p > 0),
                comment: e.comment.unwrap_or_default(),
            })
            .collect();
        if entries.is_empty() && record.paid_leave_type.is_none() {
            self.days.remove(&day);
        } else {
            self.days.insert(
                day,
                Day {
                    paid_leave_type: record.paid_leave_type,
                    paid_leave_reason: record.paid_leave_reason,
                    entries,
                },
            );
        }
        Reply::status(StatusCode::NO_CONTENT)
    }

    /// `GET Activity?unlessEndedBeforeDate=`: all activities, without the ones closed before the date.
    fn all_activities(&self, query: Option<&str>) -> Reply {
        let param = query.into_iter().flat_map(|q| q.split('&')).find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            k.eq_ignore_ascii_case("unlessEndedBeforeDate")
                .then_some(v)
                .filter(|v| !v.is_empty())
        });
        let unless_ended_before = match param.map(parse_iso) {
            None => None,
            Some(Some(date)) => Some(date),
            Some(None) => return Reply::bad_request("unlessEndedBeforeDate must be an ISO-date"),
        };
        let list = self
            .activities
            .iter()
            .filter(|a| match (a.end, unless_ended_before) {
                (Some(end), Some(date)) => end >= date,
                _ => true,
            })
            .map(|a| self.activity_json(a))
            .collect();
        Reply::json(Value::Array(list))
    }

    /// `GET Activity/suitable/{userId}/{date}`: what was booked in the 7 days before and after `date`.
    fn suitable_activities(&self, date: &str) -> Reply {
        let Some(date) = parse_iso(date) else {
            return Reply::status(StatusCode::NOT_FOUND);
        };
        let ids: BTreeSet<i32> = self
            .days
            .range(date - Duration::days(7)..date + Duration::days(7))
            .flat_map(|(_, d)| d.entries.iter().map(|e| e.activity_id))
            .collect();
        let list = ids
            .into_iter()
            .filter_map(|id| self.activity(id))
            .map(|a| self.activity_json(a))
            .collect();
        Reply::json(Value::Array(list))
    }

    fn favourite_activities(&self) -> Reply {
        let list = self
            .favourites
            .iter()
            .filter_map(|&id| self.activity(id))
            .map(|a| self.activity_json(a))
            .collect();
        Reply::json(Value::Array(list))
    }

    /// `PUT`/`DELETE activity/favourite` with an `Activity` resource as body.
    fn set_favourite(&mut self, body: &[u8], favourite: bool) -> Reply {
        let id = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|v| v.get("id").and_then(Value::as_i64))
            .and_then(|id| i32::try_from(id).ok());
        let Some(id) = id.filter(|&id| self.activity(id).is_some()) else {
            return Reply::validation_problem(&[(
                "Id".into(),
                "expected an activity of the list as body".into(),
            )]);
        };
        if favourite {
            self.favourites.insert(id);
        } else {
            self.favourites.remove(&id);
        }
        Reply::status(StatusCode::NO_CONTENT)
    }

    /// `GET /`: what the server holds right now, to check what an app changed.
    fn overview(&self) -> Value {
        let days: BTreeMap<String, Vec<Value>> = self
            .days
            .iter()
            .map(|(date, d)| {
                (
                    iso(*date),
                    d.entries.iter().map(|e| self.entry_json(e)).collect(),
                )
            })
            .collect();
        json!({
            "server": "RolX fake server (cargo xtask fake-server)",
            "editLock": iso(self.edit_lock),
            "favourites": self.favourites,
            "activities": self.activities.iter().map(|a| format!("{} {} ({})", a.id, a.full_number(), a.name)).collect::<Vec<_>>(),
            "days": days,
        })
    }
}

#[cfg(test)]
#[path = "fake_server.tests.rs"]
mod tests;
