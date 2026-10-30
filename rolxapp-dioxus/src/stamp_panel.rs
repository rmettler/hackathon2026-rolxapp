use std::time::Duration as StdDuration;

use chrono::{Duration, Local, NaiveDateTime, Timelike};
use dioxus::prelude::*;
use rolx_core::activities::{Activity, Group, search};
use rolx_core::dayplan::{fmt_clock, fmt_hm, parse_clock};
use rolx_core::idle::Away;
use rolx_core::stamp::{PendingEntry, StaleStamp, Status, most_recent_time};
use rolx_core::week::DAILY_TARGET_SECS;

use crate::activity_picker::ActivityPicker;
use crate::lucide::{Glyph, Icon};
use crate::stamper::{STALE_DISMISSED, Stamper, mutate};

const TILES: usize = 6;

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

/// Where a forgotten session most plausibly ended: a normal working day after it started.
fn suggested_end(stale: &StaleStamp) -> String {
    let end = stale.since + Duration::seconds(DAILY_TARGET_SECS);
    fmt_clock(i64::from(end.time().num_seconds_from_midnight()))
}

#[component]
fn StaleBanner(stale: StaleStamp, now: NaiveDateTime) -> Element {
    let stamper = use_context::<Stamper>();
    let mut end_text = use_signal(String::new);

    let text = if end_text().is_empty() {
        suggested_end(&stale)
    } else {
        end_text()
    };
    let end = parse_clock(&text).map(|secs| most_recent_time(secs, now));
    let end_label = end
        .map(|e| e.format("%a %d.%m. %H:%M").to_string())
        .unwrap_or_else(|| "not a time".into());
    let since = stale.since.format("%a %H:%M").to_string();
    let elapsed = fmt_hm(stale.elapsed.num_seconds());

    rsx! {
        div { class: "card banner",
            p { class: "warn glyph-text",
                Icon { glyph: Glyph::TriangleAlert }
                "You have been stamped in since {since} ({elapsed} ago). Did you forget to stamp out?"
            }
            div { class: "controls",
                label {
                    "Stopped at "
                    input {
                        r#type: "time",
                        value: "{text}",
                        oninput: move |e| end_text.set(e.value()),
                    }
                }
                span { class: "muted", "= {end_label}" }
            }
            if let Some(m) = (stamper.message)() {
                p { class: "error", "{m}" }
            }
            div { class: "controls",
                button {
                    class: "ok",
                    disabled: end.is_none(),
                    onclick: move |_| {
                        if let Some(end) = end {
                            stamper
                                .run_always(
                                    Box::new(move |s, now| {
                                        s.stamp_out_at(end, now)?;
                                        Ok(())
                                    }),
                                );
                        }
                    },
                    "Stamp out at that time"
                }
                button {
                    class: "secondary",
                    onclick: move |_| {
                        stamper
                            .run_always(
                                Box::new(|s, now| {
                                    s.stamp_out(now)?;
                                    Ok(())
                                }),
                            )
                    },
                    "Stamp out now"
                }
                button {
                    class: "danger",
                    onclick: move |_| {
                        stamper
                            .run_always(
                                Box::new(|s, _| {
                                    s.discard_open();
                                    Ok(())
                                }),
                            )
                    },
                    "Discard session"
                }
                button {
                    class: "secondary",
                    onclick: move |_| *STALE_DISMISSED.write() = true,
                    "I'm still working"
                }
            }
        }
    }
}

#[component]
fn AwayBanner(away: Signal<Option<Away>>) -> Element {
    let stamper = use_context::<Stamper>();
    let Some(a) = away() else {
        return rsx! {};
    };
    let minutes = a.length().num_minutes();
    let from = a.since.format("%H:%M").to_string();
    let until = a.until.format("%H:%M").to_string();
    let (since, until_at) = (a.since, a.until);

    rsx! {
        div { class: "card banner",
            p { class: "warn glyph-text",
                Icon { glyph: Glyph::Moon }
                "No keyboard or mouse input from {from} to {until} ({minutes} min) while you were stamped in."
            }
            div { class: "controls",
                button {
                    class: "ok",
                    onclick: move |_| {
                        away.set(None);
                        stamper
                            .run_always(
                                Box::new(move |s, now| {
                                    s.add_past_break(since, until_at, now)?;
                                    Ok(())
                                }),
                            );
                    },
                    "Take it off as a break"
                }
                button {
                    class: "secondary",
                    onclick: move |_| away.set(None),
                    "Keep it as work"
                }
                button {
                    class: "danger",
                    onclick: move |_| {
                        away.set(None);
                        stamper
                            .run_always(
                                Box::new(move |s, now| {
                                    s.stamp_out_at(since, now)?;
                                    Ok(())
                                }),
                            );
                    },
                    "Stamp out at {from}"
                }
            }
        }
    }
}

#[component]
pub fn StampPanel(
    selected: Signal<Option<i32>>,
    away: Signal<Option<Away>>,
    on_favourite: EventHandler<(i32, bool)>,
    on_reload: EventHandler<()>,
) -> Element {
    let stamper = use_context::<Stamper>();
    let state = stamper.state;
    let activities = stamper.activities;
    let mut now = use_signal(|| Local::now().naive_local());
    let mut comment = use_signal(String::new);
    let mut shown_session = use_signal(|| None::<NaiveDateTime>);
    let mut show_queue = use_signal(|| false);
    let mut discard = use_signal(|| None::<PendingEntry>);

    use_future(move || async move {
        loop {
            tokio::time::sleep(StdDuration::from_secs(1)).await;
            now.set(Local::now().naive_local());
        }
    });

    // The report field belongs to the running session: it shows that session's report and is
    // cleared when the session changes or ends. Other updates must not overwrite what is typed.
    use_effect(move || {
        let open = state.read().open.clone();
        let start = open.as_ref().map(|o| o.start);
        if start != *shown_session.peek() {
            shown_session.set(start);
            comment.set(open.and_then(|o| o.comment).unwrap_or_default());
        }
    });

    let selected_activity = move || {
        activities
            .read()
            .iter()
            .find(|a| Some(a.id) == selected())
            .map(Activity::to_ref)
    };
    let report = move || Some(comment().trim().to_string()).filter(|c| !c.is_empty());

    let current = state();
    let status = current.status(now());
    let pending = current.pending.len();
    let noun = if pending == 1 { "entry" } else { "entries" };
    let unassigned = current.unassigned_count();
    let queue: Vec<PendingEntry> = current.pending.clone();
    let stale = current.stale(now());
    let blocked = stale.is_some() && !STALE_DISMISSED();
    let syncing = (stamper.syncing)();

    let (tiles, repeat): (Vec<Activity>, Option<(Activity, Option<String>)>) = {
        let all = activities.read();
        let tiles = search(&all, &current.usage, "")
            .into_iter()
            .filter(|h| h.group != Group::Other)
            .take(TILES)
            .map(|h| h.activity.clone())
            .collect();
        let repeat = current.last_used().and_then(|(id, text)| {
            all.iter()
                .find(|a| a.id == id)
                .map(|a| (a.clone(), text.map(str::to_string)))
        });
        (tiles, repeat)
    };

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

    let repeat_id = repeat.as_ref().filter(|_| !stamped_in).map(|(a, _)| a.id);

    rsx! {
        if let Some(stale) = stale.clone() {
            if !STALE_DISMISSED() {
                StaleBanner { stale, now: now() }
            }
        }
        AwayBanner { away }
        div { class: "card stamp",
            div { class: if on_break { "timer paused" } else { "timer" }, "{timer}" }
            div { class: "status", "{headline}" }
            if !blocked {
                if (repeat.is_some() && !stamped_in) || !tiles.is_empty() {
                    div { class: "tiles",
                        span { class: "tiles-label",
                            if stamped_in {
                                "Switch to"
                            } else {
                                "Quick start"
                            }
                        }
                        if let Some((activity, text)) = repeat.clone().filter(|_| !stamped_in) {
                            button {
                                class: "tile repeat",
                                title: "Repeat last: {activity.label()}",
                                onclick: {
                                    let id = activity.id;
                                    move |_| stamper.start(id, text.clone())
                                },
                                Icon { glyph: Glyph::RotateCcw }
                                "{activity.short_label()}"
                            }
                        }
                        for tile in tiles.into_iter().filter(|t| Some(t.id) != repeat_id) {
                            button {
                                key: "{tile.id}",
                                class: "tile",
                                title: "{tile.label()}",
                                onclick: {
                                    let id = tile.id;
                                    move |_| stamper.start(id, report())
                                },
                                if tile.favourite {
                                    Icon { glyph: Glyph::Star, size: 14, filled: true }
                                }
                                "{tile.short_label()}"
                            }
                        }
                    }
                }
            }
            div { class: "form",
                ActivityPicker {
                    activities,
                    state,
                    selected,
                    on_favourite,
                    on_reload,
                    show_chips: false,
                }
                input {
                    placeholder: if stamped_in { "What did you do? (saved when you stamp out)" } else { "Work report (optional)" },
                    value: "{comment}",
                    oninput: move |e| comment.set(e.value()),
                    onchange: move |e| {
                        // Keep a report that is typed while working safe against a crash.
                        if stamped_in {
                            let text = e.value();
                            let _ = mutate(
                                Box::new(move |s, _| {
                                    s.set_open_comment(Some(text))?;
                                    Ok(())
                                }),
                            );
                        }
                    },
                }
            }
            div { class: "actions-row",
                if !stamped_in {
                    button {
                        class: "ok big",
                        disabled: blocked,
                        onclick: move |_| {
                            let activity = selected_activity();
                            let text = report();
                            stamper
                                .run(
                                    Box::new(move |s, now| {
                                        s.stamp_in(now, activity, text);
                                        Ok(())
                                    }),
                                );
                        },
                        "Stamp in"
                    }
                } else {
                    button {
                        disabled: blocked,
                        onclick: move |_| {
                            let activity = selected_activity();
                            let text = report();
                            stamper
                                .run(
                                    Box::new(move |s, now| {
                                        if let Some(text) = text {
                                            s.set_open_comment(Some(text))?;
                                        }
                                        s.switch(now, activity, None)?;
                                        Ok(())
                                    }),
                                );
                        },
                        "Switch to selected"
                    }
                    if on_break {
                        button {
                            class: "ok",
                            disabled: blocked,
                            onclick: move |_| {
                                stamper.run(Box::new(|s, now| Ok(s.end_break(now)?)))
                            },
                            "Resume"
                        }
                    } else {
                        button {
                            class: "secondary",
                            disabled: blocked,
                            onclick: move |_| {
                                stamper.run(Box::new(|s, now| Ok(s.start_break(now)?)))
                            },
                            "Break"
                        }
                    }
                    button {
                        class: "danger",
                        disabled: blocked,
                        onclick: move |_| {
                            let text = report();
                            stamper
                                .run(
                                    Box::new(move |s, now| {
                                        if let Some(text) = text {
                                            s.set_open_comment(Some(text))?;
                                        }
                                        s.stamp_out(now)?;
                                        Ok(())
                                    }),
                                );
                        },
                        "Stamp out"
                    }
                }
            }
            if pending > 0 {
                p { class: "muted",
                    "{pending} {noun} waiting to upload "
                    button {
                        class: "secondary",
                        disabled: syncing,
                        onclick: move |_| stamper.sync(false),
                        if syncing {
                            "Syncing…"
                        } else {
                            "Sync now"
                        }
                    }
                    " "
                    button {
                        class: "secondary",
                        onclick: move |_| show_queue.set(!show_queue()),
                        if show_queue() {
                            "Hide"
                        } else {
                            "Show"
                        }
                    }
                }
            }
            if show_queue() {
                div { class: "queue",
                    for (i , entry) in queue.into_iter().enumerate() {
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
                                                stamper
                                                    .run_always(
                                                        Box::new(move |s, _| {
                                                            s.remove_pending(&entry);
                                                            Ok(())
                                                        }),
                                                    );
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
                                    class: "icon small",
                                    title: "Discard this entry",
                                    "aria-label": "Discard this entry",
                                    onclick: {
                                        let entry = entry.clone();
                                        move |_| discard.set(Some(entry.clone()))
                                    },
                                    Icon { glyph: Glyph::X }
                                }
                            }
                        }
                    }
                }
            }
            if unassigned > 0 {
                p { class: "muted warn",
                    "{unassigned} without activity. Pick one above, then "
                    button {
                        class: "secondary",
                        disabled: selected_activity().is_none(),
                        onclick: move |_| {
                            if let Some(activity) = selected_activity() {
                                stamper
                                    .run_always(
                                        Box::new(move |s, now| {
                                            s.assign_unassigned(&activity, now);
                                            Ok(())
                                        }),
                                    );
                            }
                        },
                        "assign"
                    }
                }
            }
            if let Some(m) = (stamper.message)() {
                p { class: "muted", "{m}" }
            }
        }
    }
}
