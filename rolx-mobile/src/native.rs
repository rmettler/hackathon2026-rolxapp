//! Native phone features: an ongoing notification while stamped in (with a timer the system keeps running without
//! the app, "Break" and "Stamp out" buttons, and on Android 16 QPR2+ a Live Update with a status bar chip), a vibration
//! when stamping in and out, and the share sheet with texts for a day and a week. The Android parts are in
//! `android.rs`; on other targets the calls do nothing, so the app still builds and runs on the desktop.

use std::cell::Cell;
use std::rc::Rc;

use chrono::{Local, NaiveDate, NaiveDateTime};
use dioxus::logger::tracing::warn;
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity};
use rolx_core::dayplan::{DayEntry, fmt_clock, fmt_hm, sort_entries};
use rolx_core::stamp::{StampState, Status};
use rolx_core::week::WeekSummary;

use crate::links::StampLink;

/// What the "stamped in" notification shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampNotification {
    pub title: String,
    pub text: String,
    /// Epoch millis the timer counts from (now minus the time worked); `None` on a break, which shows no timer.
    pub chronometer_base_ms: Option<i64>,
    /// Text of the status bar chip of a Live Update (Android 16 QPR2+, Xiaomi's island), which otherwise shows the
    /// timer.
    pub chip_text: Option<String>,
    /// The buttons, each opening a stamp link (see `links.rs`).
    pub actions: Vec<NotificationAction>,
}

/// A button of the "stamped in" notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    pub label: &'static str,
    pub link: String,
}

impl NotificationAction {
    fn new(label: &'static str, link: StampLink) -> Self {
        Self {
            label,
            link: link.to_link(),
        }
    }
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
    // `break-start`/`break-end` rather than `break`: a button of an outdated notification must not undo a change.
    let stamp_out = NotificationAction::new("Stamp out", StampLink::Out);
    Some(match break_start {
        Some(start) if on_break => StampNotification {
            title,
            text: format!("On break since {}", time_label(start, now)),
            chronometer_base_ms: None,
            chip_text: Some("Break".into()),
            actions: vec![
                NotificationAction::new("End break", StampLink::BreakEnd),
                stamp_out,
            ],
        },
        _ => StampNotification {
            title,
            text: activity.map_or_else(
                || "no activity yet".into(),
                |a| activity_label(a.id, &a.full_number, activities),
            ),
            chronometer_base_ms: Some(now_ms - worked.num_milliseconds()),
            chip_text: None,
            actions: vec![
                NotificationAction::new("Break", StampLink::BreakStart),
                stamp_out,
            ],
        },
    })
}

/// `08:12`, with the weekday if it was not today (a forgotten stamp-out).
pub(crate) fn time_label(at: NaiveDateTime, now: NaiveDateTime) -> String {
    if at.date() == now.date() {
        at.format("%H:%M").to_string()
    } else {
        at.format("%a %H:%M").to_string()
    }
}

/// The activity's full number and name if it is in `activities`, else the number alone.
pub(crate) fn activity_label(id: i32, number: &str, activities: &[Activity]) -> String {
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

/// Feedback for a stamp action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vibration {
    /// A short pulse.
    StampIn,
    /// Two short pulses.
    StampOut,
}

impl Vibration {
    /// Alternately milliseconds off and on, starting with off.
    fn timings_ms(self) -> &'static [i64] {
        match self {
            Vibration::StampIn => &[0, 60],
            Vibration::StampOut => &[0, 60, 120, 60],
        }
    }
}

/// Vibrates for a stamp action. Does nothing off Android and on devices without a vibrator.
pub fn vibrate(kind: Vibration) -> anyhow::Result<()> {
    platform::vibrate(kind.timings_ms())
}

/// Keeps the notification in line with the app's stamp state, once at start (the app may have been killed while
/// stamped in) and on every change, and vibrates when stamping in or out (not at start). Call it once, in the root
/// component, with `AppState::stamp_state`. Errors are logged, not shown.
pub fn use_native_stamp_sync(state: Signal<StampState>) {
    // Not signals: the effect must not depend on them.
    let was_stamped_in = use_hook(|| Rc::new(Cell::new(None::<bool>)));
    // Whether the permission request has been answered is not reported to the app, so it is polled.
    let polling = use_hook(|| Rc::new(Cell::new(false)));
    use_effect(move || {
        let stamped_in = state.read().open.is_some();
        log_error(
            "updating the stamp notification",
            update_stamp_notification(&state.read()),
        );
        if let Some(before) = was_stamped_in.replace(Some(stamped_in))
            && before != stamped_in
        {
            let kind = if stamped_in {
                Vibration::StampIn
            } else {
                Vibration::StampOut
            };
            log_error("vibrating", vibrate(kind));
        }
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

/// Whether the user lets the app show its notification as a Live Update (status bar chip, Xiaomi's island; Android
/// 16 QPR2+, turned off per app in the system settings). `None` where there are no Live Updates.
pub fn live_updates_allowed() -> Option<bool> {
    platform::live_updates_allowed()
}

/// Opens the system settings page that turns the app's Live Updates on or off.
pub fn open_live_update_settings() -> anyhow::Result<()> {
    platform::open_live_update_settings()
}

/// How many seconds to wait for the answer to the permission request.
const PERMISSION_POLLS: u32 = 120;

/// Opens the share sheet to send `text` as plain text (`subject` is used by mail and the like). Does nothing off
/// Android.
pub fn share_text(subject: &str, text: &str) -> anyhow::Result<()> {
    platform::share_text(subject, text)
}

/// A text to share, e.g. with [`share_text`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareText {
    pub subject: String,
    pub text: String,
}

/// The entries of a day as text: one line per entry (time span if any, length, activity, report) and the total.
/// `activities` gives the activities' names.
pub fn day_share(date: NaiveDate, entries: &[DayEntry], activities: &[Activity]) -> ShareText {
    let mut entries = entries.to_vec();
    sort_entries(&mut entries);
    let title = format!("RolX {}", date.format("%a %d.%m.%Y"));
    let total: i64 = entries.iter().map(|e| e.duration).sum();

    let mut lines = vec![title.clone()];
    if entries.is_empty() {
        lines.push("No entries.".into());
    }
    for e in &entries {
        let span = e
            .begin
            .zip(e.end())
            .map(|(begin, end)| format!("{}-{} · ", fmt_clock(i64::from(begin)), fmt_clock(end)));
        let report = e.comment.as_deref().filter(|c| !c.trim().is_empty());
        lines.push(format!(
            "{}{} · {}{}",
            span.unwrap_or_default(),
            fmt_hm(e.duration),
            activity_label(e.activity_id, &e.full_activity_number, activities),
            report.map(|r| format!(" · {r}")).unwrap_or_default(),
        ));
    }
    lines.push(format!("Total {}", fmt_hm(total)));
    ShareText {
        subject: title,
        text: lines.join("\n"),
    }
}

/// A week summary as text: booked against the target, each working day, and the time per activity.
/// `activities` gives the activities' names.
pub fn week_share(summary: &WeekSummary, activities: &[Activity]) -> ShareText {
    let title = match (summary.days.first(), summary.days.last()) {
        (Some(first), Some(last)) => format!(
            "RolX week {}-{}",
            first.date.format("%d.%m."),
            last.date.format("%d.%m.%Y")
        ),
        _ => "RolX week".to_string(),
    };
    let left = match summary.remaining_secs {
        s if s >= 0 => format!("{} left", fmt_hm(s)),
        s => format!("{} over", fmt_hm(-s)),
    };

    let mut lines = vec![
        title.clone(),
        format!(
            "Booked {} of {} · {left}",
            fmt_hm(summary.booked_secs),
            fmt_hm(summary.target_secs)
        ),
    ];
    if summary.leave_secs > 0 {
        lines.push(format!("Leave {}", fmt_hm(summary.leave_secs)));
    }
    lines.push(String::new());
    // Weekends and holidays only when something was booked on them.
    for day in summary
        .days
        .iter()
        .filter(|d| d.nominal_secs > 0 || d.booked_secs > 0)
    {
        lines.push(format!(
            "{} {} of {}{}",
            day.date.format("%a %d.%m."),
            fmt_hm(day.booked_secs),
            fmt_hm(day.nominal_secs),
            if day.on_leave { " · leave" } else { "" },
        ));
    }
    if !summary.activities.is_empty() {
        lines.push(String::new());
        for total in &summary.activities {
            // `week::summarize` books queued entries without an activity on id 0.
            let label = if total.activity_id == 0 {
                "no activity yet".to_string()
            } else {
                activity_label(total.activity_id, &total.number, activities)
            };
            lines.push(format!("{} · {label}", fmt_hm(total.secs)));
        }
    }
    ShareText {
        subject: title,
        text: lines.join("\n"),
    }
}

pub(crate) fn log_error(what: &str, result: anyhow::Result<()>) {
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
        let actions: Vec<(&str, &str)> = content
            .actions
            .iter()
            .map(|a| (a.label, a.link.as_str()))
            .collect();
        android::show_stamp_notification(&android::StampNotificationContent {
            title: &content.title,
            text: &content.text,
            chronometer_base_ms: content.chronometer_base_ms,
            chip_text: content.chip_text.as_deref(),
            actions: &actions,
        })?;
        Ok(())
    }

    pub fn live_updates_allowed() -> Option<bool> {
        android::promoted_notifications_allowed().ok().flatten()
    }

    pub fn open_live_update_settings() -> anyhow::Result<()> {
        android::open_promoted_notification_settings()
    }

    pub fn permission_requested() -> bool {
        PERMISSION_ASKED.load(Ordering::Relaxed)
    }

    pub fn vibrate(timings_ms: &[i64]) -> anyhow::Result<()> {
        android::vibrate(timings_ms)
    }

    pub fn share_text(subject: &str, text: &str) -> anyhow::Result<()> {
        android::share_text(subject, text)
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

    pub fn live_updates_allowed() -> Option<bool> {
        None
    }

    pub fn open_live_update_settings() -> anyhow::Result<()> {
        Ok(())
    }

    pub fn permission_requested() -> bool {
        false
    }

    pub fn vibrate(_timings_ms: &[i64]) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn share_text(_subject: &str, _text: &str) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn notifications_allowed() -> bool {
        false
    }
}

#[cfg(test)]
#[path = "native.tests.rs"]
mod tests;
