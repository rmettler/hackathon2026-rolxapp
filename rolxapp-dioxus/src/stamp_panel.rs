use std::time::Duration as StdDuration;

use chrono::{Duration, Local, NaiveDateTime};
use dioxus::prelude::*;
use rolx_core::activities::{self, Activity};
use rolx_core::dayplan::{fmt_clock, fmt_hm};
use rolx_core::stamp::{PendingEntry, StampState, Status};
use rolx_core::sync::{self, SyncReport};

use crate::activity_picker::ActivityPicker;
use crate::shared;

type Action = Box<dyn FnOnce(&mut StampState, NaiveDateTime) -> anyhow::Result<()>>;

/// Applies an action to the state on disk (not to the copy in memory), so changes made by the
/// CLI in the meantime are never overwritten.
fn mutate(action: Action) -> anyhow::Result<StampState> {
    let mut state = StampState::load()?;
    action(&mut state, Local::now().naive_local())?;
    state.save()?;
    Ok(state)
}

/// Uploads the queue. `None` when there is no valid login, since stamping works without one.
async fn sync_all() -> anyhow::Result<Option<SyncReport>> {
    if !shared::has_session() {
        return Ok(None);
    }
    let client = shared::client().await?;
    Ok(Some(sync::run_default(&client).await?))
}

fn fmt_hms(d: Duration) -> String {
    let s = d.num_seconds().max(0);
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn queue_line(e: &PendingEntry) -> String {
    format!(
        "{} {}-{} · {} · {}{}",
        e.date.format("%d.%m."),
        fmt_clock(i64::from(e.begin)),
        fmt_clock(i64::from(e.begin) + e.duration + i64::from(e.pause)),
        fmt_hm(e.duration),
        e.activity
            .as_ref()
            .map_or("no activity yet", |a| a.full_number.as_str()),
        e.comment
            .as_ref()
            .map(|c| format!(" · {c}"))
            .unwrap_or_default()
    )
}

#[component]
pub fn StampPanel(
    state: Signal<StampState>,
    activities: Signal<Vec<Activity>>,
    selected: Signal<Option<i32>>,
    on_uploaded: EventHandler<()>,
    on_favourite: EventHandler<(i32, bool)>,
    on_reload: EventHandler<()>,
) -> Element {
    let mut now = use_signal(|| Local::now().naive_local());
    let mut comment = use_signal(String::new);
    let mut message = use_signal(|| None::<String>);
    let mut syncing = use_signal(|| false);
    let mut show_queue = use_signal(|| false);
    let mut discard = use_signal(|| None::<PendingEntry>);

    use_future(move || async move {
        loop {
            tokio::time::sleep(StdDuration::from_secs(1)).await;
            now.set(Local::now().naive_local());
        }
    });

    let selected_activity = move || {
        activities
            .read()
            .iter()
            .find(|a| Some(a.id) == selected())
            .map(Activity::to_ref)
    };

    // `quiet` is for the automatic upload after stamping: no message unless something went wrong.
    let start_sync = move |quiet: bool| {
        spawn(async move {
            syncing.set(true);
            match sync_all().await {
                Ok(Some(report)) => {
                    if let Some((day, reason)) = report.failed.first() {
                        message.set(Some(format!("{day} not uploaded: {reason}")));
                    } else if !quiet {
                        message.set(Some(format!("Uploaded {} entries", report.uploaded)));
                    } else {
                        message.set(None);
                    }
                }
                Ok(None) if !quiet => message.set(Some("Log in to upload your entries".into())),
                Ok(None) => {}
                Err(e) => message.set(Some(format!("Upload failed, will retry later: {e}"))),
            }
            state.set(StampState::load().unwrap_or_default());
            activities.set(activities::load_cached().unwrap_or_default());
            syncing.set(false);
            on_uploaded.call(());
        });
    };

    let mut act = move |action: Action| match mutate(action) {
        Ok(new_state) => {
            message.set(None);
            let has_uploadable = new_state.pending.iter().any(|e| e.activity.is_some());
            state.set(new_state);
            if has_uploadable {
                start_sync(true);
            }
        }
        Err(e) => message.set(Some(e.to_string())),
    };

    let current = state();
    let status = current.status(now());
    let pending = current.pending.len();
    let noun = if pending == 1 { "entry" } else { "entries" };
    let unassigned = current.unassigned_count();
    let queue: Vec<PendingEntry> = current.pending.clone();

    let (timer, headline, stamped_in, on_break) = match &status {
        Status::StampedOut => (
            "00:00:00".to_string(),
            "Stamped out".to_string(),
            false,
            false,
        ),
        Status::StampedIn {
            since,
            activity,
            on_break,
            worked,
        } => (
            fmt_hms(*worked),
            format!(
                "Since {}{}{}",
                since.format("%H:%M"),
                activity
                    .as_ref()
                    .map(|a| format!(" · {}", a.full_number))
                    .unwrap_or_else(|| " · no activity yet".into()),
                if *on_break { " · on break" } else { "" }
            ),
            true,
            *on_break,
        ),
    };

    rsx! {
        div { class: "card stamp",
            div { class: if on_break { "timer paused" } else { "timer" }, "{timer}" }
            div { class: "status", "{headline}" }
            ActivityPicker { activities, state, selected, on_favourite, on_reload }
            div { class: "controls",
                input {
                    placeholder: "Work report (optional)",
                    value: "{comment}",
                    oninput: move |e| comment.set(e.value()),
                }
            }
            div { class: "controls",
                if !stamped_in {
                    button {
                        class: "ok",
                        onclick: move |_| {
                            let activity = selected_activity();
                            let text = Some(comment()).filter(|c| !c.trim().is_empty());
                            act(Box::new(move |s, now| {
                                s.stamp_in(now, activity, text);
                                Ok(())
                            }));
                        },
                        "Stamp in"
                    }
                } else {
                    button {
                        onclick: move |_| {
                            let activity = selected_activity();
                            let text = Some(comment()).filter(|c| !c.trim().is_empty());
                            act(Box::new(move |s, now| {
                                s.switch(now, activity, text)?;
                                Ok(())
                            }));
                        },
                        "Switch to selected"
                    }
                    if on_break {
                        button {
                            class: "ok",
                            onclick: move |_| act(Box::new(|s, now| Ok(s.end_break(now)?))),
                            "Resume"
                        }
                    } else {
                        button {
                            class: "secondary",
                            onclick: move |_| act(Box::new(|s, now| Ok(s.start_break(now)?))),
                            "Break"
                        }
                    }
                    button {
                        class: "danger",
                        onclick: move |_| act(Box::new(|s, now| {
                            s.stamp_out(now)?;
                            Ok(())
                        })),
                        "Stamp out"
                    }
                }
            }
            if pending > 0 {
                p { class: "muted",
                    "{pending} {noun} waiting to upload "
                    button {
                        class: "secondary",
                        disabled: syncing(),
                        onclick: move |_| start_sync(false),
                        if syncing() { "Syncing…" } else { "Sync now" }
                    }
                    " "
                    button {
                        class: "secondary",
                        onclick: move |_| show_queue.set(!show_queue()),
                        if show_queue() { "Hide" } else { "Show" }
                    }
                }
            }
            if show_queue() {
                div { class: "queue",
                    for (i, entry) in queue.into_iter().enumerate() {
                        div { key: "{i}", class: "queue-row",
                            span { "{queue_line(&entry)}" }
                            if discard() == Some(entry.clone()) {
                                span {
                                    "Discard? "
                                    button {
                                        class: "danger",
                                        onclick: {
                                            let entry = entry.clone();
                                            move |_| {
                                                discard.set(None);
                                                let entry = entry.clone();
                                                act(Box::new(move |s, _| {
                                                    s.remove_pending(&entry);
                                                    Ok(())
                                                }));
                                            }
                                        },
                                        "Yes"
                                    }
                                    " "
                                    button {
                                        class: "secondary",
                                        onclick: move |_| discard.set(None),
                                        "No"
                                    }
                                }
                            } else {
                                button {
                                    class: "secondary",
                                    title: "Discard this entry",
                                    onclick: {
                                        let entry = entry.clone();
                                        move |_| discard.set(Some(entry.clone()))
                                    },
                                    "✕"
                                }
                            }
                        }
                    }
                }
            }
            if unassigned > 0 {
                p { class: "muted warn",
                    "{unassigned} without activity — pick one above, then "
                    button {
                        class: "secondary",
                        disabled: selected_activity().is_none(),
                        onclick: move |_| {
                            if let Some(activity) = selected_activity() {
                                act(Box::new(move |s, now| {
                                    s.assign_unassigned(&activity, now);
                                    Ok(())
                                }));
                            }
                        },
                        "assign"
                    }
                }
            }
            if let Some(m) = message() {
                p { class: "muted", "{m}" }
            }
        }
    }
}
