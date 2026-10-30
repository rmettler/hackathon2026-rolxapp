use chrono::NaiveDate;
use dioxus::prelude::*;
use rolx_core::activities::Activity;
use rolx_core::dayedit::save_day;
use rolx_core::dayplan::{
    DayEntry, EntryChange, Fix, Issue, analyze, apply_change, apply_fix, fix_new_issues, fmt_clock,
    fmt_hm, new_issues, parse_clock, sort_entries,
};
use rolx_core::stamp::StampState;
use rolx_core::week::DAILY_TARGET_SECS;

use crate::activity_picker::ActivityPicker;
use crate::shared;

/// An edit that would make entries overlap, waiting for the user to decide what to do about it.
#[derive(Clone, PartialEq)]
struct Conflict {
    base: Vec<DayEntry>,
    new: Vec<DayEntry>,
    details: Vec<String>,
}

fn time_range(e: &DayEntry) -> String {
    match (e.begin, e.end()) {
        (Some(b), Some(end)) => format!("{}–{}", fmt_clock(i64::from(b)), fmt_clock(end)),
        _ => "no time".into(),
    }
}

fn clock_or_empty(secs: Option<i64>) -> String {
    secs.map(fmt_clock).unwrap_or_default()
}

/// The entries of one day with edit, delete, and conflict resolution. Changes are only written
/// if the day on the server is still what was shown (see `save_day`).
#[component]
pub fn DayView(
    day: NaiveDate,
    entries: Vec<DayEntry>,
    state: Signal<StampState>,
    activities: Signal<Vec<Activity>>,
    on_favourite: EventHandler<(i32, bool)>,
    on_reload: EventHandler<()>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut editing = use_signal(|| None::<usize>);
    let mut confirm_delete = use_signal(|| None::<usize>);
    let mut form_activity = use_signal(|| None::<i32>);
    let mut form_comment = use_signal(String::new);
    let mut form_begin = use_signal(String::new);
    let mut form_end = use_signal(String::new);
    let mut form_pause = use_signal(String::new);
    let mut form_pause_initial = use_signal(String::new);
    let mut conflict = use_signal(|| None::<Conflict>);
    let mut message = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let save = move |base: Vec<DayEntry>, new: Vec<DayEntry>| {
        spawn(async move {
            busy.set(true);
            let result = async {
                let client = shared::client().await?;
                save_day(&client, day, &base, &new).await
            }
            .await;
            match result {
                Ok(()) => {
                    editing.set(None);
                    confirm_delete.set(None);
                    conflict.set(None);
                    message.set(None);
                    on_changed.call(());
                }
                Err(e) => message.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let mut queued: Vec<DayEntry> = state
        .read()
        .pending
        .iter()
        .filter(|p| p.date == day)
        .map(DayEntry::from)
        .collect();
    sort_entries(&mut queued);
    let server_len = entries.len();
    let all: Vec<DayEntry> = entries.iter().chain(&queued).cloned().collect();
    let label = move |i: usize| {
        if i < server_len {
            format!("#{}", i + 1)
        } else {
            format!("queued {}", i - server_len + 1)
        }
    };
    let issues: Vec<(Issue, String)> = analyze(&all)
        .into_iter()
        .map(|i| (i, i.describe(&label)))
        .collect();

    let total_secs: i64 = all.iter().map(|e| e.duration).sum();
    let total = fmt_hm(total_secs);
    // The day target is always 8.4 h, independent of pensum.
    let target_note = if total_secs <= DAILY_TARGET_SECS {
        format!(
            "{} left of {}",
            fmt_hm(DAILY_TARGET_SECS - total_secs),
            fmt_hm(DAILY_TARGET_SECS)
        )
    } else {
        format!(
            "{} over {}",
            fmt_hm(total_secs - DAILY_TARGET_SECS),
            fmt_hm(DAILY_TARGET_SECS)
        )
    };

    let rows: Vec<(usize, DayEntry)> = entries.iter().cloned().enumerate().collect();

    rsx! {
        div { class: "card",
            if all.is_empty() {
                p { class: "empty", "No entries for this day. Target {fmt_hm(DAILY_TARGET_SECS)}." }
            } else {
                table {
                    thead {
                        tr {
                            th { "#" }
                            th { "Time" }
                            th { "Activity" }
                            th { "Work" }
                            th { "Comment" }
                            th {}
                        }
                    }
                    tbody {
                        for (i , e) in rows {
                            if editing() == Some(i) {
                                tr { key: "e{i}", class: "editing",
                                    td { colspan: "6",
                                        ActivityPicker {
                                            activities,
                                            state,
                                            selected: form_activity,
                                            on_favourite,
                                            on_reload,
                                        }
                                        div { class: "controls",
                                            label { "From "
                                                input {
                                                    r#type: "time",
                                                    value: "{form_begin}",
                                                    oninput: move |ev| form_begin.set(ev.value()),
                                                }
                                            }
                                            label { "To "
                                                input {
                                                    r#type: "time",
                                                    value: "{form_end}",
                                                    oninput: move |ev| form_end.set(ev.value()),
                                                }
                                            }
                                            label { "Break "
                                                input {
                                                    class: "short",
                                                    r#type: "number",
                                                    min: "0",
                                                    placeholder: "0",
                                                    value: "{form_pause}",
                                                    oninput: move |ev| form_pause.set(ev.value()),
                                                }
                                                " min"
                                            }
                                            input {
                                                placeholder: "Work report",
                                                value: "{form_comment}",
                                                oninput: move |ev| form_comment.set(ev.value()),
                                            }
                                        }
                                        div { class: "controls",
                                            button {
                                                class: "ok",
                                                disabled: busy(),
                                                onclick: {
                                                    let base = entries.clone();
                                                    move |_| {
                                                        message.set(None);
                                                        let parse = |text: String| -> Result<Option<i32>, String> {
                                                            if text.trim().is_empty() {
                                                                return Ok(None);
                                                            }
                                                            parse_clock(&text)
                                                                .map(Some)
                                                                .ok_or_else(|| format!("'{text}' is not a time like 09:30"))
                                                        };
                                                        let (begin, end) = match (parse(form_begin()), parse(form_end())) {
                                                            (Ok(b), Ok(e)) => (b, e),
                                                            (Err(m), _) | (_, Err(m)) => {
                                                                message.set(Some(m));
                                                                return;
                                                            }
                                                        };
                                                        let activity = form_activity()
                                                            .and_then(|id| {
                                                                activities
                                                                    .read()
                                                                    .iter()
                                                                    .find(|a| a.id == id)
                                                                    .map(|a| (a.id, a.full_number.clone()))
                                                            });
                                                        // Only touch the break if the field was changed, so a break with
                                                        // odd seconds survives an unrelated edit.
                                                        let pause = if form_pause().trim() == form_pause_initial().trim() {
                                                            None
                                                        } else {
                                                            match form_pause().trim() {
                                                                "" => Some(0),
                                                                text => match text.parse::<u32>() {
                                                                    Ok(minutes) if minutes < 24 * 60 => Some(minutes as i32 * 60),
                                                                    _ => {
                                                                        message.set(Some("The break must be a number of minutes below 24 h".into()));
                                                                        return;
                                                                    }
                                                                },
                                                            }
                                                        };
                                                        let change = EntryChange {
                                                            activity,
                                                            comment: Some(Some(form_comment())),
                                                            begin,
                                                            end,
                                                            pause,
                                                        };
                                                        let mut new = base.clone();
                                                        if let Err(err) = apply_change(&mut new, i, &change) {
                                                            message.set(Some(err.to_string()));
                                                            return;
                                                        }
                                                        if new == base {
                                                            editing.set(None);
                                                            return;
                                                        }
                                                        let overlaps: Vec<String> = new_issues(&base, &new)
                                                            .iter()
                                                            .filter(|i| i.is_overlap())
                                                            .map(|i| i.describe(&|n| format!("#{}", n + 1)))
                                                            .collect();
                                                        if overlaps.is_empty() {
                                                            save(base.clone(), new);
                                                        } else {
                                                            conflict
                                                                .set(
                                                                    Some(Conflict {
                                                                        base: base.clone(),
                                                                        new,
                                                                        details: overlaps,
                                                                    }),
                                                                );
                                                        }
                                                    }
                                                },
                                                "Save"
                                            }
                                            button {
                                                class: "secondary",
                                                onclick: move |_| {
                                                    editing.set(None);
                                                    conflict.set(None);
                                                    message.set(None);
                                                },
                                                "Cancel"
                                            }
                                        }
                                        if let Some(c) = conflict() {
                                            div { class: "conflict",
                                                p { class: "warn",
                                                    "This change makes entries overlap:"
                                                }
                                                ul {
                                                    for d in c.details.iter() {
                                                        li { "{d}" }
                                                    }
                                                }
                                                div { class: "controls",
                                                    for (title , fix) in [
                                                        ("Shorten the earlier entry", Fix::TruncateFirst),
                                                        ("Move the later entry", Fix::ShiftSecond),
                                                    ] {
                                                        button {
                                                            key: "{title}",
                                                            disabled: busy(),
                                                            onclick: {
                                                                let c = c.clone();
                                                                move |_| {
                                                                    let mut new = c.new.clone();
                                                                    match fix_new_issues(&mut new, &c.base, fix) {
                                                                        Ok(()) => save(c.base.clone(), new),
                                                                        Err(err) => message.set(Some(err.to_string())),
                                                                    }
                                                                }
                                                            },
                                                            "{title}"
                                                        }
                                                    }
                                                    button {
                                                        class: "secondary",
                                                        disabled: busy(),
                                                        onclick: {
                                                            let c = c.clone();
                                                            move |_| save(c.base.clone(), c.new.clone())
                                                        },
                                                        "Save anyway"
                                                    }
                                                    button {
                                                        class: "secondary",
                                                        onclick: move |_| conflict.set(None),
                                                        "Back"
                                                    }
                                                }
                                            }
                                        }
                                        if let Some(m) = message() {
                                            p { class: "error", "{m}" }
                                        }
                                    }
                                }
                            } else {
                                tr { key: "r{i}",
                                    td { class: "num", "#{i + 1}" }
                                    td { class: "num", "{time_range(&e)}" }
                                    td { class: "activity", title: "{e.full_activity_number}", "{e.full_activity_number}" }
                                    td { class: "num",
                                        "{fmt_hm(e.duration)}"
                                        if let Some(p) = e.pause.filter(|p| *p > 0) {
                                            span { class: "muted", " +{fmt_hm(i64::from(p))} break" }
                                        }
                                    }
                                    td { class: "comment", "{e.comment.clone().unwrap_or_default()}" }
                                    td { class: "actions",
                                        if confirm_delete() == Some(i) {
                                            "Delete? "
                                            button {
                                                class: "danger",
                                                disabled: busy(),
                                                onclick: {
                                                    let base = entries.clone();
                                                    move |_| {
                                                        let mut new = base.clone();
                                                        new.remove(i);
                                                        save(base.clone(), new);
                                                    }
                                                },
                                                "Yes"
                                            }
                                            " "
                                            button {
                                                class: "secondary",
                                                onclick: move |_| confirm_delete.set(None),
                                                "No"
                                            }
                                        } else {
                                            button {
                                                class: "secondary",
                                                onclick: {
                                                    let e = e.clone();
                                                    move |_| {
                                                        form_activity.set(Some(e.activity_id));
                                                        form_comment.set(e.comment.clone().unwrap_or_default());
                                                        form_begin.set(clock_or_empty(e.begin.map(i64::from)));
                                                        form_end.set(clock_or_empty(e.end()));
                                                        let minutes = e
                                                            .pause
                                                            .filter(|p| *p > 0)
                                                            .map(|p| ((p + 30) / 60).to_string())
                                                            .unwrap_or_default();
                                                        form_pause_initial.set(minutes.clone());
                                                        form_pause.set(minutes);
                                                        conflict.set(None);
                                                        message.set(None);
                                                        confirm_delete.set(None);
                                                        editing.set(Some(i));
                                                    }
                                                },
                                                "Edit"
                                            }
                                            " "
                                            button {
                                                class: "secondary",
                                                onclick: move |_| {
                                                    message.set(None);
                                                    editing.set(None);
                                                    confirm_delete.set(Some(i));
                                                },
                                                "Delete"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        for (k , q) in queued.iter().enumerate() {
                            tr { key: "q{k}", class: "queued",
                                td { class: "num", "q{k + 1}" }
                                td { class: "num", "{time_range(q)}" }
                                td {
                                    if q.full_activity_number.is_empty() {
                                        "(no activity yet)"
                                    } else {
                                        "{q.full_activity_number}"
                                    }
                                }
                                td { class: "num", "{fmt_hm(q.duration)}" }
                                td { class: "muted", "waiting to upload" }
                                td {}
                            }
                        }
                        tr { class: "total",
                            td {}
                            td {}
                            td { "Total" }
                            td { class: "num", "{total}" }
                            td { class: "muted", "{target_note}" }
                            td {}
                        }
                    }
                }
            }
            if !issues.is_empty() {
                div { class: "issues",
                    for (n , (issue , text)) in issues.into_iter().enumerate() {
                        div { key: "i{n}", class: if issue.is_overlap() { "issue warn" } else { "issue" },
                            if issue.is_overlap() {
                                "⚠ {text}"
                            } else {
                                "· {text}"
                            }
                            if let Issue::Gap { first, second, .. } = issue {
                                if first < server_len && second < server_len {
                                    button {
                                        class: "secondary",
                                        disabled: busy(),
                                        title: "Add the unbooked time to the break of the earlier entry",
                                        onclick: {
                                            let base = entries.clone();
                                            move |_| {
                                                let mut new = base.clone();
                                                match apply_fix(&mut new, &issue, Fix::FillAsBreak) {
                                                    Ok(()) => save(base.clone(), new),
                                                    Err(err) => message.set(Some(err.to_string())),
                                                }
                                            }
                                        },
                                        "Count as break"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if editing().is_none() {
                if let Some(m) = message() {
                    p { class: "error", "{m}" }
                }
            }
        }
    }
}
