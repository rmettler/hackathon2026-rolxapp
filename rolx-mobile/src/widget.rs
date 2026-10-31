//! What the home-screen widget shows (`StampWidget.kt` in the `plugin` module draws it): the running session with a
//! timer, or "Stamped out" with the activity a tap would stamp in on.

use chrono::NaiveDateTime;
use rolx_core::activities::Activity;
use rolx_core::stamp::{StampState, Status};
use serde::Serialize;

use crate::native::{activity_label, time_label};

/// The widget's content, sent to Kotlin as JSON (camelCase).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetView {
    /// Whether the button stamps out (else in).
    pub stamped_in: bool,
    /// The small line on top.
    pub title: String,
    /// Shows the time worked in this session (`worked_ms`) instead of `headline`.
    pub timer: bool,
    /// Whether the timer runs; it stands still on a break.
    pub running: bool,
    pub worked_ms: i64,
    pub headline: String,
    /// The small line below.
    pub text: String,
    /// A forgotten session (see `StampState::stale`): the button opens the app instead of stamping out.
    pub stale: bool,
}

/// The widget for `state` at the local time `now`; `activities` gives the activities' names.
pub fn widget_view(state: &StampState, now: NaiveDateTime, activities: &[Activity]) -> WidgetView {
    let Status::StampedIn {
        since,
        activity,
        on_break,
        worked,
    } = state.status(now)
    else {
        // The widget stamps in like the hotkey: on the activity used last, if it is still in the list.
        let next = state
            .last_used()
            .and_then(|(id, _)| activities.iter().find(|a| a.id == id));
        return WidgetView {
            stamped_in: false,
            title: "RolX".into(),
            timer: false,
            running: false,
            worked_ms: 0,
            headline: "Stamped out".into(),
            text: next.map_or_else(
                || "Tap to stamp in".into(),
                |a| format!("Next: {}", a.label()),
            ),
            stale: false,
        };
    };
    let activity_text = activity.map_or_else(
        || "no activity yet".into(),
        |a| activity_label(a.id, &a.full_number, activities),
    );
    if state.stale(now).is_some() {
        return WidgetView {
            stamped_in: true,
            title: format!("Stamped in since {}", time_label(since, now)),
            timer: false,
            running: false,
            worked_ms: worked.num_milliseconds(),
            headline: "Forgot to stamp out?".into(),
            text: "Open RolX to end the session".into(),
            stale: true,
        };
    }
    let break_start = state.open.as_ref().and_then(|o| o.break_started);
    let title = match break_start {
        Some(start) if on_break => format!("On break since {}", time_label(start, now)),
        _ => format!("Stamped in since {}", time_label(since, now)),
    };
    WidgetView {
        stamped_in: true,
        title,
        timer: true,
        running: !on_break,
        worked_ms: worked.num_milliseconds(),
        headline: String::new(),
        text: activity_text,
        stale: false,
    }
}

#[cfg(test)]
#[path = "widget.tests.rs"]
mod tests;
