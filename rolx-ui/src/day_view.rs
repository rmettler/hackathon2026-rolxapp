use chrono::{Local, NaiveDate, Timelike};
use dioxus::prelude::*;
use rolx_core::activities::Activity;
use rolx_core::dayedit::{previous_entries, save_day_tracked};
use rolx_core::dayplan::{
    DayEntry, EntryChange, Fix, Issue, NewEntry, add_entry, analyze, apply_change, apply_fix,
    fix_new_issues, fmt_clock, fmt_hm, new_issues, parse_clock, sort_entries, suggest_start,
};
use rolx_core::stamp::StampState;
use rolx_core::week::DAILY_TARGET_SECS;

use crate::activity_picker::ActivityPicker;
use crate::{shared, use_compact};

/// How far back "copy the last working day" looks.
const COPY_LOOKBACK_DAYS: i64 = 7;

/// An edit that would make entries overlap, waiting for the user to decide what to do about it.
#[derive(Clone, PartialEq)]
struct Conflict {
    base: Vec<DayEntry>,
    new: Vec<DayEntry>,
    details: Vec<String>,
    what: String,
}

/// A problem between entries with its description and, for a gap, the unbooked time.
type IssueRow = (Issue, String, Option<(i32, i32)>);

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Idle,
    Edit(usize),
    Add,
}

/// The input fields of the entry editor, shared by editing and adding.
#[derive(Clone, Copy, PartialEq)]
struct Form {
    activity: Signal<Option<i32>>,
    comment: Signal<String>,
    begin: Signal<String>,
    end: Signal<String>,
    pause: Signal<String>,
    pause_initial: Signal<String>,
}

impl Form {
    fn new() -> Self {
        Self {
            activity: use_signal(|| None),
            comment: use_signal(String::new),
            begin: use_signal(String::new),
            end: use_signal(String::new),
            pause: use_signal(String::new),
            pause_initial: use_signal(String::new),
        }
    }

    fn fill_from(mut self, e: &DayEntry) {
        let minutes = e
            .pause
            .filter(|p| *p > 0)
            .map(|p| ((p + 30) / 60).to_string())
            .unwrap_or_default();
        self.activity.set(Some(e.activity_id));
        self.comment.set(e.comment.clone().unwrap_or_default());
        self.begin.set(clock_or_empty(e.begin.map(i64::from)));
        self.end.set(clock_or_empty(e.end()));
        self.pause_initial.set(minutes.clone());
        self.pause.set(minutes);
    }

    fn fill_new(mut self, activity: Option<i32>, begin: String, end: String) {
        self.activity.set(activity);
        self.comment.set(String::new());
        self.begin.set(begin);
        self.end.set(end);
        self.pause.set(String::new());
        self.pause_initial.set(String::new());
    }
}

fn plural_entries(n: usize) -> &'static str {
    if n == 1 { "entry" } else { "entries" }
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

fn parse_time_field(text: &str) -> Result<Option<i32>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    parse_clock(text)
        .map(Some)
        .ok_or_else(|| format!("'{text}' is not a time like 09:30"))
}

fn parse_break_minutes(text: &str) -> Result<i32, String> {
    match text.trim() {
        "" => Ok(0),
        text => match text.parse::<u32>() {
            Ok(minutes) if minutes < 24 * 60 => Ok(minutes as i32 * 60),
            _ => Err("The break must be a number of minutes below 24 h".into()),
        },
    }
}

fn find_activity(activities: &[Activity], id: Option<i32>) -> Option<(i32, String)> {
    let id = id?;
    activities
        .iter()
        .find(|a| a.id == id)
        .map(|a| (a.id, a.full_number.clone()))
}

/// The day's entries after applying the form to entry `index`.
fn build_edit(
    form: &Form,
    activities: &[Activity],
    base: &[DayEntry],
    index: usize,
) -> Result<Vec<DayEntry>, String> {
    let begin = parse_time_field(&(form.begin)())?;
    let end = parse_time_field(&(form.end)())?;
    // Only touch the break if the field was changed, so a break with odd seconds survives an
    // unrelated edit.
    let pause = if (form.pause)().trim() == (form.pause_initial)().trim() {
        None
    } else {
        Some(parse_break_minutes(&(form.pause)())?)
    };
    let change = EntryChange {
        activity: find_activity(activities, (form.activity)()),
        comment: Some(Some((form.comment)())),
        begin,
        end,
        pause,
    };
    let mut new = base.to_vec();
    apply_change(&mut new, index, &change).map_err(|e| e.to_string())?;
    Ok(new)
}

/// The day's entries after adding the entry described by the form.
fn build_add(
    form: &Form,
    activities: &[Activity],
    base: &[DayEntry],
) -> Result<Vec<DayEntry>, String> {
    let activity = find_activity(activities, (form.activity)())
        .ok_or_else(|| "Choose an activity".to_string())?;
    let begin =
        parse_time_field(&(form.begin)())?.ok_or_else(|| "Enter the start time".to_string())?;
    let end = parse_time_field(&(form.end)())?.ok_or_else(|| "Enter the end time".to_string())?;
    let entry = NewEntry {
        activity,
        begin,
        end,
        pause: parse_break_minutes(&(form.pause)())?,
        comment: Some((form.comment)()),
    };
    let mut new = base.to_vec();
    add_entry(&mut new, &entry).map_err(|e| e.to_string())?;
    Ok(new)
}

/// Picker, times, break and report of one entry, with the buttons to save it, and the choices to
/// make when saving would make entries overlap.
#[component]
fn EntryEditor(
    form: Form,
    activities: Signal<Vec<Activity>>,
    state: Signal<StampState>,
    on_favourite: EventHandler<(i32, bool)>,
    on_reload: EventHandler<()>,
    busy: bool,
    save_label: String,
    message: Option<String>,
    conflict: Option<Conflict>,
    on_save: EventHandler<()>,
    on_cancel: EventHandler<()>,
    on_fix: EventHandler<Fix>,
    on_save_anyway: EventHandler<()>,
    on_back: EventHandler<()>,
) -> Element {
    let Form {
        activity,
        mut comment,
        mut begin,
        mut end,
        mut pause,
        ..
    } = form;
    rsx! {
        ActivityPicker {
            activities,
            state,
            selected: activity,
            on_favourite,
            on_reload,
        }
        div { class: "controls",
            label {
                "From "
                input {
                    r#type: "time",
                    value: "{begin}",
                    oninput: move |ev| begin.set(ev.value()),
                }
            }
            label {
                "To "
                input {
                    r#type: "time",
                    value: "{end}",
                    oninput: move |ev| end.set(ev.value()),
                }
            }
            label {
                "Break "
                input {
                    class: "short",
                    r#type: "number",
                    min: "0",
                    placeholder: "0",
                    value: "{pause}",
                    oninput: move |ev| pause.set(ev.value()),
                }
                " min"
            }
            input {
                placeholder: "Work report",
                value: "{comment}",
                oninput: move |ev| comment.set(ev.value()),
            }
        }
        div { class: "controls",
            button {
                class: "ok",
                disabled: busy,
                onclick: move |_| on_save.call(()),
                "{save_label}"
            }
            button {
                class: "secondary",
                onclick: move |_| on_cancel.call(()),
                "Cancel"
            }
        }
        if let Some(c) = conflict {
            div { class: "conflict",
                p { class: "warn", "This change makes entries overlap:" }
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
                            disabled: busy,
                            onclick: move |_| on_fix.call(fix),
                            "{title}"
                        }
                    }
                    button {
                        class: "secondary",
                        disabled: busy,
                        onclick: move |_| on_save_anyway.call(()),
                        "Save anyway"
                    }
                    button {
                        class: "secondary",
                        onclick: move |_| on_back.call(()),
                        "Back"
                    }
                }
            }
        }
        if let Some(m) = message {
            p { class: "error", "{m}" }
        }
    }
}

/// The entries of one day with edit, add, delete, and conflict resolution. Changes are only
/// written if the day on the server is still what was shown (see `save_day`), and every change can
/// be undone.
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
    let mut mode = use_signal(|| Mode::Idle);
    let mut confirm_delete = use_signal(|| None::<usize>);
    let form = Form::new();
    let mut conflict = use_signal(|| None::<Conflict>);
    let mut message = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let mut copy_offer = use_signal(|| None::<(NaiveDate, Vec<DayEntry>)>);
    let compact = use_compact();

    // On a phone the editor may open far below where it was asked for (e.g. "Book it…"), so bring it into view.
    use_effect(move || {
        if compact && mode() != Mode::Idle {
            document::eval(
                "setTimeout(() => document.querySelector('.compact .editing')?.scrollIntoView({ block: 'start', behavior: 'smooth' }), 50)",
            );
        }
    });

    let save = move |base: Vec<DayEntry>, new: Vec<DayEntry>, what: String| {
        spawn(async move {
            busy.set(true);
            let result = async {
                let client = shared::client().await?;
                save_day_tracked(&client, day, &base, &new, &what).await
            }
            .await;
            match result {
                Ok(()) => {
                    mode.set(Mode::Idle);
                    confirm_delete.set(None);
                    conflict.set(None);
                    copy_offer.set(None);
                    message.set(None);
                    on_changed.call(());
                }
                Err(e) => message.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    // Saves, or asks what to do first if the new entries would overlap.
    let mut submit =
        move |built: Result<Vec<DayEntry>, String>, base: Vec<DayEntry>, what: String| {
            message.set(None);
            let new = match built {
                Ok(new) => new,
                Err(m) => {
                    message.set(Some(m));
                    return;
                }
            };
            if new == base {
                mode.set(Mode::Idle);
                return;
            }
            let overlaps: Vec<String> = new_issues(&base, &new)
                .iter()
                .filter(|i| i.is_overlap())
                .map(|i| i.describe(&|n| format!("#{}", n + 1)))
                .collect();
            if overlaps.is_empty() {
                save(base, new, what);
            } else {
                conflict.set(Some(Conflict {
                    base,
                    new,
                    details: overlaps,
                    what,
                }));
            }
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
    let issues: Vec<IssueRow> = analyze(&all)
        .into_iter()
        .map(|i| (i, i.describe(&label), i.gap_range(&all)))
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

    // What a new entry starts with: the end of the day so far, up to now if that is today, and
    // the activity used last.
    let last_activity = move || {
        let id = state.read().last_used().map(|(id, _)| id);
        id.filter(|id| activities.read().iter().any(|a| a.id == *id))
    };
    let new_entry_defaults = {
        let all = all.clone();
        move || {
            let begin = suggest_start(&all);
            let now = Local::now().time().num_seconds_from_midnight() as i32;
            let end = (day == Local::now().date_naive() && begin.is_none_or(|b| now > b + 60))
                .then(|| fmt_clock(i64::from(now)));
            (
                last_activity(),
                begin.map(i64::from).map(fmt_clock).unwrap_or_default(),
                end.unwrap_or_default(),
            )
        }
    };

    let mut start_edit = move |i: usize, e: &DayEntry| {
        form.fill_from(e);
        conflict.set(None);
        message.set(None);
        confirm_delete.set(None);
        mode.set(Mode::Edit(i));
    };
    let mut ask_delete = move |i: usize| {
        message.set(None);
        mode.set(Mode::Idle);
        confirm_delete.set(Some(i));
    };
    let delete = {
        let base = entries.clone();
        move |i: usize| {
            let mut new = base.clone();
            new.remove(i);
            save(base.clone(), new, format!("Deleted #{}", i + 1));
        }
    };
    let edit_editor = {
        let base = entries.clone();
        move |i: usize| {
            let base = base.clone();
            rsx! {
                EntryEditor {
                    form,
                    activities,
                    state,
                    on_favourite,
                    on_reload,
                    busy: busy(),
                    save_label: "Save",
                    message: message(),
                    conflict: conflict(),
                    on_save: move |_| {
                        let built = build_edit(&form, &activities.read(), &base, i);
                        submit(built, base.clone(), format!("Edited #{}", i + 1));
                    },
                    on_cancel: move |_| {
                        mode.set(Mode::Idle);
                        conflict.set(None);
                        message.set(None);
                    },
                    on_fix: move |fix| {
                        if let Some(c) = conflict() {
                            let mut new = c.new.clone();
                            match fix_new_issues(&mut new, &c.base, fix) {
                                Ok(()) => save(c.base.clone(), new, c.what.clone()),
                                Err(err) => message.set(Some(err.to_string())),
                            }
                        }
                    },
                    on_save_anyway: move |_| {
                        if let Some(c) = conflict() {
                            save(c.base.clone(), c.new.clone(), c.what.clone());
                        }
                    },
                    on_back: move |_| conflict.set(None),
                }
            }
        }
    };
    // Cards also name the activity: there is room below the number, but no hover to explain it.
    let activity_name = move |id: i32| {
        activities
            .read()
            .iter()
            .find(|a| a.id == id)
            .map(Activity::short_label)
    };

    let rows: Vec<(usize, DayEntry)> = entries.iter().cloned().enumerate().collect();
    let adding = mode() == Mode::Add;

    let find_previous = move |_| {
        spawn(async move {
            busy.set(true);
            message.set(None);
            let found = async {
                let client = shared::client().await?;
                previous_entries(&client, day, COPY_LOOKBACK_DAYS).await
            }
            .await;
            match found {
                Ok(Some(offer)) => copy_offer.set(Some(offer)),
                Ok(None) => message.set(Some(format!(
                    "Nothing to copy: no entries in the last {COPY_LOOKBACK_DAYS} days"
                ))),
                Err(e) => message.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "card",
            div { class: "card-head",
                h2 { "Entries" }
                button {
                    class: "secondary small",
                    disabled: adding,
                    onclick: {
                        let defaults = new_entry_defaults.clone();
                        move |_| {
                            let (activity, begin, end) = defaults();
                            form.fill_new(activity, begin, end);
                            conflict.set(None);
                            message.set(None);
                            confirm_delete.set(None);
                            mode.set(Mode::Add);
                        }
                    },
                    "+ Add entry"
                }
            }
            if all.is_empty() {
                p { class: "empty", "No entries for this day. Target {fmt_hm(DAILY_TARGET_SECS)}." }
                if let Some((from, copy)) = copy_offer() {
                    div { class: "conflict",
                        p {
                            "Copy {copy.len()} {plural_entries(copy.len())} ({fmt_hm(copy.iter().map(|e| e.duration).sum())}) from {from.format(\"%a %d.%m.\")}?"
                        }
                        ul {
                            for (n , e) in copy.iter().enumerate() {
                                li { key: "c{n}", "{time_range(e)} · {e.full_activity_number} · {fmt_hm(e.duration)}" }
                            }
                        }
                        div { class: "controls",
                            button {
                                class: "ok",
                                disabled: busy(),
                                onclick: {
                                    let copy = copy.clone();
                                    move |_| {
                                        let what = format!(
                                            "Copied {} {} from {from}",
                                            copy.len(),
                                            plural_entries(copy.len()),
                                        );
                                        save(Vec::new(), copy.clone(), what);
                                    }
                                },
                                "Copy"
                            }
                            button {
                                class: "secondary",
                                onclick: move |_| copy_offer.set(None),
                                "Cancel"
                            }
                        }
                    }
                } else if !adding {
                    div { class: "controls",
                        button {
                            class: "secondary",
                            disabled: busy(),
                            onclick: find_previous,
                            "Copy the last working day"
                        }
                    }
                }
            } else if compact {
                div { class: "entries",
                    for (i , e) in rows.clone() {
                        if mode() == Mode::Edit(i) {
                            div { key: "e{i}", class: "entry editing", {edit_editor(i)} }
                        } else {
                            div { key: "r{i}", class: "entry",
                                div { class: "entry-top",
                                    span { class: "num", "{time_range(&e)}" }
                                    span { class: "num",
                                        if let Some(p) = e.pause.filter(|p| *p > 0) {
                                            span { class: "muted", "+{fmt_hm(i64::from(p))} break  " }
                                        }
                                        strong { "{fmt_hm(e.duration)}" }
                                    }
                                }
                                div { class: "entry-activity",
                                    "{e.full_activity_number}"
                                    if let Some(name) = activity_name(e.activity_id) {
                                        span { class: "muted", " · {name}" }
                                    }
                                }
                                if let Some(c) = e.comment.clone().filter(|c| !c.is_empty()) {
                                    div { class: "entry-comment", "{c}" }
                                }
                                if confirm_delete() == Some(i) {
                                    div { class: "entry-actions confirm",
                                        span { class: "warn", "Delete this entry?" }
                                        button {
                                            class: "danger",
                                            disabled: busy(),
                                            onclick: {
                                                let delete = delete.clone();
                                                move |_| delete(i)
                                            },
                                            "Delete"
                                        }
                                        button {
                                            class: "secondary",
                                            onclick: move |_| confirm_delete.set(None),
                                            "Keep"
                                        }
                                    }
                                } else {
                                    div { class: "entry-actions",
                                        button {
                                            class: "secondary",
                                            onclick: {
                                                let e = e.clone();
                                                move |_| start_edit(i, &e)
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "secondary del",
                                            onclick: move |_| ask_delete(i),
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                    for (k , q) in queued.iter().enumerate() {
                        div { key: "q{k}", class: "entry queued",
                            div { class: "entry-top",
                                span { class: "num", "{time_range(q)}" }
                                strong { class: "num", "{fmt_hm(q.duration)}" }
                            }
                            div { class: "entry-activity",
                                if q.full_activity_number.is_empty() {
                                    "(no activity yet)"
                                } else {
                                    "{q.full_activity_number}"
                                }
                            }
                            div { class: "muted", "waiting to upload" }
                        }
                    }
                    div { class: "entry-total",
                        span { "Total " }
                        strong { class: "num", "{total}" }
                        span { class: "muted", "{target_note}" }
                    }
                }
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
                            if mode() == Mode::Edit(i) {
                                tr { key: "e{i}", class: "editing",
                                    td { colspan: "6", {edit_editor(i)} }
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
                                                    let delete = delete.clone();
                                                    move |_| delete(i)
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
                                                class: "quiet",
                                                onclick: {
                                                    let e = e.clone();
                                                    move |_| start_edit(i, &e)
                                                },
                                                "Edit"
                                            }
                                            " "
                                            button {
                                                class: "quiet del",
                                                onclick: move |_| ask_delete(i),
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
            if adding {
                div { class: "editing add",
                    h3 { "New entry" }
                    EntryEditor {
                        form,
                        activities,
                        state,
                        on_favourite,
                        on_reload,
                        busy: busy(),
                        save_label: "Add",
                        message: message(),
                        conflict: conflict(),
                        on_save: {
                            let base = entries.clone();
                            move |_| {
                                let built = build_add(&form, &activities.read(), &base);
                                let what = match find_activity(&activities.read(), (form.activity)()) {
                                    Some((_, number)) => format!("Added {number}"),
                                    None => "Added an entry".to_string(),
                                };
                                submit(built, base.clone(), what);
                            }
                        },
                        on_cancel: move |_| {
                            mode.set(Mode::Idle);
                            conflict.set(None);
                            message.set(None);
                        },
                        on_fix: move |fix| {
                            if let Some(c) = conflict() {
                                let mut new = c.new.clone();
                                match fix_new_issues(&mut new, &c.base, fix) {
                                    Ok(()) => save(c.base.clone(), new, c.what.clone()),
                                    Err(err) => message.set(Some(err.to_string())),
                                }
                            }
                        },
                        on_save_anyway: move |_| {
                            if let Some(c) = conflict() {
                                save(c.base.clone(), c.new.clone(), c.what.clone());
                            }
                        },
                        on_back: move |_| conflict.set(None),
                    }
                }
            }
            if !issues.is_empty() {
                div { class: "issues",
                    for (n , (issue , text , gap)) in issues.into_iter().enumerate() {
                        div { key: "i{n}", class: if issue.is_overlap() { "issue warn" } else { "issue" },
                            if issue.is_overlap() {
                                "⚠ {text}"
                            } else {
                                "· {text}"
                            }
                            if let Some((from, to)) = gap {
                                button {
                                    class: "secondary",
                                    disabled: busy() || adding,
                                    title: "Book an entry for this time",
                                    onclick: {
                                        let defaults = new_entry_defaults.clone();
                                        move |_| {
                                            let (activity, _, _) = defaults();
                                            form.fill_new(activity, fmt_clock(i64::from(from)), fmt_clock(i64::from(to)));
                                            conflict.set(None);
                                            message.set(None);
                                            confirm_delete.set(None);
                                            mode.set(Mode::Add);
                                        }
                                    },
                                    "Book it…"
                                }
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
                                                    Ok(()) => save(base.clone(), new, "Counted a gap as break".to_string()),
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
            if mode() == Mode::Idle {
                if let Some(m) = message() {
                    p { class: "error", "{m}" }
                }
            }
        }
    }
}
