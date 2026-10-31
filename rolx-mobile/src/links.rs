//! Stamp links and the app shortcuts that use them.
//!
//! A stamp link (`rolx://stamp?action=out`) stamps in or out from outside the app: a scanned QR code, Tasker, an
//! Assistant routine that opens a link, or the app shortcuts (long press on the app icon: "Stamp in: Meetings",
//! "Stamp out", "Break"). `StampLinkActivity` in the `plugin` module receives them, queues them in a file and opens the
//! app, which applies them like a tap on the stamp panel: on the stamp state on disk, refused while a forgotten
//! session waits for an answer, with the usual vibration and upload. On other targets there are no links or
//! shortcuts.
//!
//! The link: `rolx://stamp?action=<action>&activity=<activity>`, both optional.
//! - `in` (the default): stamps in with the activity, else with the one used last. When already stamped in, switches
//!   to the activity (if another one) and ends a break.
//! - `out`: stamps out.
//! - `break`: starts a break, or ends the one running. `break-start` and `break-end` only do the one.
//! - `toggle`: stamps out when stamped in, else in as `in` without an activity.
//!
//! `activity` is an id, a full number (`%231001.001.01`, as `#` starts the fragment) or words that match one activity
//! only (`Meetings`), as in the CLI's `--activity`.

use std::collections::BTreeMap;

use anyhow::{anyhow, bail};
use chrono::{Local, NaiveDateTime};
use dioxus::logger::tracing::{info, warn};
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity, Group, search};
use rolx_core::stamp::{ActivityRef, StampError, StampState, Usage};
use rolx_ui::stamper::Stamper;
use tokio::sync::Notify;

use crate::native::log_error;

const PREFIX: &str = "rolx://stamp";
/// The queue `StampLinkActivity` writes, in the app's files dir: one `<epoch millis> <link>` per line.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const QUEUE_FILE: &str = "stamp-links.txt";
/// Older links in the queue are dropped: the app did not start in time (e.g. it crashed), and stamping at a later
/// time than asked would book wrong times.
const MAX_AGE_MS: i64 = 2 * 60 * 1000;
/// How many "Stamp in: …" shortcuts there are at most (favourites, then recent activities). With "Stamp out" and
/// "Break" that makes four, which every launcher shows.
const STAMP_IN_SHORTCUTS: usize = 2;

/// What a stamp link asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampLink {
    /// Stamp in, with the activity matching the query or else the one used last.
    In {
        activity: Option<String>,
    },
    Out,
    /// Start a break, or end the one running.
    Break,
    BreakStart,
    BreakEnd,
    /// Stamp out when stamped in, else in.
    Toggle,
}

impl StampLink {
    pub fn parse(link: &str) -> anyhow::Result<Self> {
        let link = link.trim();
        let rest = link
            .get(..PREFIX.len())
            .filter(|p| p.eq_ignore_ascii_case(PREFIX))
            .map(|_| &link[PREFIX.len()..])
            .ok_or_else(|| anyhow!("not a stamp link ({PREFIX}?…)"))?;
        let rest = rest.split('#').next().unwrap_or_default();
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        let query = match rest {
            "" => "",
            _ => rest
                .strip_prefix('?')
                .ok_or_else(|| anyhow!("not a stamp link ({PREFIX}?…)"))?,
        };

        let mut action = None;
        let mut activity = None;
        for pair in query.split('&').filter(|p| !p.is_empty()) {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            // `+` is a space in query strings, as QR code generators write them.
            let value = urlencoding::decode(&value.replace('+', " "))?
                .trim()
                .to_string();
            match key {
                "action" => action = Some(value.to_lowercase()),
                "activity" => activity = Some(value).filter(|v| !v.is_empty()),
                _ => bail!("unknown parameter '{key}', use action and activity"),
            }
        }

        let parsed = match action.as_deref().unwrap_or("in") {
            "in" => return Ok(StampLink::In { activity }),
            "out" => StampLink::Out,
            "break" => StampLink::Break,
            "break-start" => StampLink::BreakStart,
            "break-end" => StampLink::BreakEnd,
            "toggle" => StampLink::Toggle,
            other => bail!(
                "unknown action '{other}', use in, out, break, break-start, break-end or toggle"
            ),
        };
        if activity.is_some() {
            bail!("only action=in takes an activity");
        }
        Ok(parsed)
    }

    pub fn to_link(&self) -> String {
        let action = match self {
            StampLink::In { activity: None } => "in",
            StampLink::In {
                activity: Some(activity),
            } => {
                return format!(
                    "{PREFIX}?action=in&activity={}",
                    urlencoding::encode(activity)
                );
            }
            StampLink::Out => "out",
            StampLink::Break => "break",
            StampLink::BreakStart => "break-start",
            StampLink::BreakEnd => "break-end",
            StampLink::Toggle => "toggle",
        };
        format!("{PREFIX}?action={action}")
    }

    /// Applies the link to `state` at `now`; `activities` resolves the activity. Asking for what already is (stamp in
    /// with the running activity, start a break on a break) changes nothing; what can't be done is an error.
    pub fn apply(
        &self,
        state: &mut StampState,
        activities: &[Activity],
        now: NaiveDateTime,
    ) -> anyhow::Result<()> {
        let on_break = state.open.as_ref().map(|o| o.break_started.is_some());
        match self {
            StampLink::In { activity } => {
                let activity = match activity {
                    Some(query) => Some(activities::resolve(activities, query)?.to_ref()),
                    None => None,
                };
                stamp_in(state, activity, activities, now)?;
            }
            StampLink::Out => {
                state.stamp_out(now)?;
            }
            StampLink::Break | StampLink::BreakStart | StampLink::BreakEnd => {
                match (self, on_break.ok_or(StampError::NotStampedIn)?) {
                    (StampLink::Break | StampLink::BreakEnd, true) => state.end_break(now)?,
                    (StampLink::Break | StampLink::BreakStart, false) => state.start_break(now)?,
                    _ => {}
                }
            }
            StampLink::Toggle if on_break.is_some() => {
                state.stamp_out(now)?;
            }
            StampLink::Toggle => stamp_in(state, None, activities, now)?,
        }
        Ok(())
    }
}

/// Stamps in with `activity`, else with the one used last; when stamped in, switches to `activity` if it is another
/// one and ends a break.
fn stamp_in(
    state: &mut StampState,
    activity: Option<ActivityRef>,
    activities: &[Activity],
    now: NaiveDateTime,
) -> anyhow::Result<()> {
    let Some((running, on_break)) = state.open.as_ref().map(|o| {
        let running = o.activity.as_ref().map(|a| a.id);
        (running, o.break_started.is_some())
    }) else {
        let activity = activity
            .or_else(|| activities::last_used(activities, &state.usage).map(Activity::to_ref));
        state.stamp_in(now, activity, None);
        return Ok(());
    };
    match activity {
        Some(activity) if running != Some(activity.id) => {
            state.switch(now, Some(activity), None)?;
        }
        _ if on_break => state.end_break(now)?,
        _ => {}
    }
    Ok(())
}

/// The links of a queue file (`<epoch millis> <link>` per line) that are at most [`MAX_AGE_MS`] old at `now_ms`.
pub fn fresh_links(queue: &str, now_ms: i64) -> Vec<String> {
    queue
        .lines()
        .filter_map(|line| {
            let (at, link) = line.trim().split_once(' ')?;
            let at: i64 = at.parse().ok()?;
            if now_ms - at > MAX_AGE_MS {
                warn!(link, age_ms = now_ms - at, "dropping an old stamp link");
                return None;
            }
            Some(link.trim().to_string())
        })
        .collect()
}

/// An app shortcut, see [`shortcuts`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutSpec {
    pub id: String,
    pub short_label: String,
    pub long_label: String,
    /// A drawable in the `plugin` module.
    pub icon: &'static str,
    pub link: String,
}

/// The app shortcuts, in their order: "Stamp in: <activity>" for the first favourites and recent activities (as the
/// quick start tiles; a plain "Stamp in" without any), "Stamp out" and "Break".
pub fn shortcuts(activities: &[Activity], usage: &BTreeMap<i32, Usage>) -> Vec<ShortcutSpec> {
    let mut list: Vec<ShortcutSpec> = search(activities, usage, "")
        .into_iter()
        .filter(|h| h.group != Group::Other)
        .take(STAMP_IN_SHORTCUTS)
        .map(|h| {
            let name = h.activity.short_label();
            ShortcutSpec {
                id: format!("stamp-in-{}", h.activity.id),
                long_label: format!("Stamp in: {name}"),
                short_label: name,
                icon: "ic_shortcut_stamp_in",
                link: StampLink::In {
                    activity: Some(h.activity.id.to_string()),
                }
                .to_link(),
            }
        })
        .collect();
    if list.is_empty() {
        list.push(shortcut(
            "stamp-in",
            "Stamp in",
            "ic_shortcut_stamp_in",
            StampLink::In { activity: None },
        ));
    }
    list.push(shortcut(
        "stamp-out",
        "Stamp out",
        "ic_shortcut_stamp_out",
        StampLink::Out,
    ));
    list.push(shortcut(
        "break",
        "Break",
        "ic_shortcut_break",
        StampLink::Break,
    ));
    list
}

fn shortcut(id: &str, label: &str, icon: &'static str, link: StampLink) -> ShortcutSpec {
    ShortcutSpec {
        id: id.into(),
        short_label: label.into(),
        long_label: label.into(),
        icon,
        link: link.to_link(),
    }
}

/// Wakes up [`use_stamp_links`] to read the queue.
static WAKE: Notify = Notify::const_new();

/// Applies the queued stamp links, at start and whenever `StampLinkActivity` queues one, calling `on_link` first (e.g.
/// to show the stamp panel); keeps the app shortcuts in line with the activities. Call it once, in the root component.
pub fn use_stamp_links(stamper: Stamper, on_link: Callback<()>) {
    use_future(move || async move {
        log_error(
            "listening for stamp links",
            platform::listen(|| WAKE.notify_one()),
        );
        loop {
            match platform::take_queue() {
                Ok(queue) => {
                    for link in fresh_links(&queue, Local::now().timestamp_millis()) {
                        on_link.call(());
                        handle(stamper, &link);
                    }
                }
                Err(e) => warn!(error = format!("{e:#}"), "reading the stamp links failed"),
            }
            WAKE.notified().await;
        }
    });

    let shortcuts =
        use_memo(move || shortcuts(&stamper.activities.read(), &stamper.state.read().usage));
    use_effect(move || {
        log_error(
            "setting the app shortcuts",
            platform::set_shortcuts(&shortcuts.read()),
        );
    });
}

/// Reads the queue again, in case a wake-up was missed (e.g. the app was not listening yet).
pub fn check_queue() {
    WAKE.notify_one();
}

fn handle(mut stamper: Stamper, link: &str) {
    info!(link, "stamp link");
    match StampLink::parse(link) {
        Ok(parsed) => {
            let activities = stamper.activities.peek().clone();
            stamper.run(Box::new(move |s, now| parsed.apply(s, &activities, now)));
        }
        Err(e) => stamper
            .message
            .set(Some(format!("Cannot stamp with {link}: {e}"))),
    }
}

#[cfg(target_os = "android")]
mod platform {
    use std::fs;
    use std::io::ErrorKind;

    use super::{QUEUE_FILE, ShortcutSpec};
    use crate::android;

    pub fn listen(on_queued: fn()) -> anyhow::Result<()> {
        android::listen_for_stamp_links(on_queued)
    }

    /// Takes the queue's content and empties it. Moved away first, so a link queued meanwhile goes to a new file.
    pub fn take_queue() -> anyhow::Result<String> {
        let path = android::files_dir()?.join(QUEUE_FILE);
        let taken = path.with_extension("taken");
        match fs::rename(&path, &taken) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(String::new()),
            Err(e) => return Err(e.into()),
        }
        let queue = fs::read_to_string(&taken)?;
        fs::remove_file(&taken)?;
        Ok(queue)
    }

    pub fn set_shortcuts(shortcuts: &[ShortcutSpec]) -> anyhow::Result<()> {
        let shortcuts: Vec<android::Shortcut> = shortcuts
            .iter()
            .map(|s| android::Shortcut {
                id: &s.id,
                short_label: &s.short_label,
                long_label: &s.long_label,
                icon: s.icon,
                link: &s.link,
            })
            .collect();
        android::set_shortcuts(&shortcuts)
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use super::ShortcutSpec;

    pub fn listen(_on_queued: fn()) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn take_queue() -> anyhow::Result<String> {
        Ok(String::new())
    }

    pub fn set_shortcuts(_shortcuts: &[ShortcutSpec]) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "links.tests.rs"]
mod tests;
