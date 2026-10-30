//! Native phone features around stamping: an ongoing notification while stamped in (with a timer the system keeps
//! running without the app). The Android parts are in `android.rs`; on other targets the calls do nothing, so the
//! app still builds and runs on the desktop.

#![allow(
    dead_code,
    reason = "API for the app's stamping UI; not all of it is wired up yet"
)]

use std::cell::Cell;
use std::rc::Rc;

use chrono::{Local, NaiveDateTime};
use dioxus::logger::tracing::warn;
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity};
use rolx_core::stamp::{StampState, Status};

/// What the "stamped in" notification shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampNotification {
    pub title: String,
    pub text: String,
    /// Epoch millis the timer counts from (now minus the time worked); `None` on a break, which shows no timer.
    pub chronometer_base_ms: Option<i64>,
}

/// The notification for `state`, or `None` when stamped out. `now` is the local time and `now_ms` the same moment in
/// epoch millis; `activities` gives the activity's name.
pub fn stamp_notification(
    state: &StampState,
    now: NaiveDateTime,
    now_ms: i64,
    activities: &[Activity],
) -> Option<StampNotification> {
    let Status::StampedIn {
        since,
        activity,
        on_break,
        worked,
    } = state.status(now)
    else {
        return None;
    };
    let title = format!("Stamped in since {}", time_label(since, now));
    let break_start = state.open.as_ref().and_then(|o| o.break_started);
    Some(match break_start {
        Some(start) if on_break => StampNotification {
            title,
            text: format!("On break since {}", time_label(start, now)),
            chronometer_base_ms: None,
        },
        _ => StampNotification {
            title,
            text: activity.map_or_else(
                || "no activity yet".into(),
                |a| activity_label(a.id, &a.full_number, activities),
            ),
            chronometer_base_ms: Some(now_ms - worked.num_milliseconds()),
        },
    })
}

/// `08:12`, with the weekday if it was not today (a forgotten stamp-out).
fn time_label(at: NaiveDateTime, now: NaiveDateTime) -> String {
    if at.date() == now.date() {
        at.format("%H:%M").to_string()
    } else {
        at.format("%a %H:%M").to_string()
    }
}

/// The activity's full number and name if it is in `activities`, else the number alone.
fn activity_label(id: i32, number: &str, activities: &[Activity]) -> String {
    match activities.iter().find(|a| a.id == id) {
        Some(a) => a.label(),
        None if number.is_empty() => format!("activity {id}"),
        None => number.to_string(),
    }
}

/// Shows, updates or removes the ongoing "stamped in" notification to match `state`. On Android 13+ the first
/// stamp-in asks for the notification permission; without it nothing is shown. Does nothing off Android.
pub fn update_stamp_notification(state: &StampState) -> anyhow::Result<()> {
    let activities = activities::load_cached().unwrap_or_default();
    let now = Local::now();
    let content = stamp_notification(
        state,
        now.naive_local(),
        now.timestamp_millis(),
        &activities,
    );
    platform::update_notification(content.as_ref())
}

/// Keeps the notification in line with the app's stamp state: once at start (the app may have been killed while
/// stamped in) and on every change. Call it once, in the root component, with `AppState::stamp_state`. Errors are
/// logged, not shown.
pub fn use_native_stamp_sync(state: Signal<StampState>) {
    // Whether the permission request has been answered is not reported to the app, so it is polled.
    let polling = use_hook(|| Rc::new(Cell::new(false)));
    use_effect(move || {
        log_error(
            "updating the stamp notification",
            update_stamp_notification(&state.read()),
        );
        if platform::permission_requested() && !polling.replace(true) {
            spawn(wait_for_permission(state));
        }
    });
}

/// After asking for the notification permission: shows the notification as soon as it is granted.
async fn wait_for_permission(state: Signal<StampState>) {
    for _ in 0..PERMISSION_POLLS {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if platform::notifications_allowed() {
            log_error(
                "updating the stamp notification",
                update_stamp_notification(&state.peek()),
            );
            return;
        }
    }
}

/// How many seconds to wait for the answer to the permission request.
const PERMISSION_POLLS: u32 = 120;

fn log_error(what: &str, result: anyhow::Result<()>) {
    if let Err(e) = result {
        warn!(error = format!("{e:#}"), "{what} failed");
    }
}

#[cfg(target_os = "android")]
mod platform {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::StampNotification;
    use crate::android;

    /// The permission is asked for once per app start, at the first stamp-in (or at start when stamped in).
    static PERMISSION_ASKED: AtomicBool = AtomicBool::new(false);

    pub fn update_notification(content: Option<&StampNotification>) -> anyhow::Result<()> {
        let Some(content) = content else {
            return android::cancel_stamp_notification();
        };
        if !android::notifications_allowed()? {
            if !PERMISSION_ASKED.swap(true, Ordering::Relaxed) {
                android::request_notification_permission()?;
            }
            return Ok(());
        }
        android::show_stamp_notification(
            &content.title,
            &content.text,
            content.chronometer_base_ms,
        )?;
        Ok(())
    }

    pub fn permission_requested() -> bool {
        PERMISSION_ASKED.load(Ordering::Relaxed)
    }

    pub fn notifications_allowed() -> bool {
        android::notifications_allowed().unwrap_or(false)
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use super::StampNotification;

    pub fn update_notification(_content: Option<&StampNotification>) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn permission_requested() -> bool {
        false
    }

    pub fn notifications_allowed() -> bool {
        false
    }
}

#[cfg(test)]
#[path = "native.tests.rs"]
mod tests;
