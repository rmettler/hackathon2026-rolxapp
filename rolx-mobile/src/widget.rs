//! What the home-screen widget shows (`StampWidget.kt` in the `plugin` module draws it): the running session with a
//! timer, or "Stamped out" with the activity a tap would stamp in on. Also its activity picker
//! (`ActivityPicker.kt`): the list to choose from and what choosing does.

use chrono::NaiveDateTime;
use rolx_core::activities::{self, Activity, Group};
use rolx_core::stamp::{ActivityRef, StampError, StampState, Status};
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
                || "Choose an activity".into(),
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

/// An activity in the widget's picker, sent to Kotlin as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PickerItem {
    pub id: i32,
    /// The last part of the name, e.g. "Meetings".
    pub short: String,
    pub label: String,
    /// `favourite`, `recent` or `other`.
    pub group: &'static str,
    /// The activity of the running session.
    pub current: bool,
}

/// How many activities the picker lists; typing narrows it down.
pub const PICKER_LIMIT: usize = 60;

/// The activities matching `query`, ranked like the app's picker: favourites, recently used, then the rest.
pub fn picker_items(all: &[Activity], state: &StampState, query: &str) -> Vec<PickerItem> {
    let current = state
        .open
        .as_ref()
        .and_then(|o| o.activity.as_ref())
        .map(|a| a.id);
    activities::search(all, &state.usage, query)
        .into_iter()
        .take(PICKER_LIMIT)
        .map(|hit| PickerItem {
            id: hit.activity.id,
            short: hit.activity.short_label(),
            label: hit.activity.label(),
            group: match hit.group {
                Group::Favourite => "favourite",
                Group::Recent => "recent",
                Group::Other => "other",
            },
            current: current == Some(hit.activity.id),
        })
        .collect()
}

/// What [`start_on`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    /// Stamped in on it.
    StampedIn,
    /// Ended the running session and continued on it.
    Switched,
    /// It is the activity of the running session already, or one without an activity got it.
    Unchanged,
}

/// Picking an activity in the widget: stamps in on it, or continues on it when stamped in. A running session without
/// an activity gets it instead of being split, as the app's "assign" does.
pub fn start_on(
    state: &mut StampState,
    now: NaiveDateTime,
    activity: ActivityRef,
) -> Result<Started, StampError> {
    let Some(open) = &state.open else {
        state.stamp_in(now, Some(activity), None);
        return Ok(Started::StampedIn);
    };
    match &open.activity {
        Some(current) if current.id == activity.id => Ok(Started::Unchanged),
        Some(_) => {
            state.switch(now, Some(activity), None)?;
            Ok(Started::Switched)
        }
        None => {
            state.assign_unassigned(&activity, now);
            Ok(Started::Unchanged)
        }
    }
}

#[cfg(test)]
#[path = "widget.tests.rs"]
mod tests;
