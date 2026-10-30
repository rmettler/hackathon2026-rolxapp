use std::time::Duration as StdDuration;

use chrono::{Datelike, Local, NaiveDate, Timelike};
use dioxus::prelude::*;
use rolx_core::balance::fetch_balance;
use rolx_core::dayplan::fmt_hm;
use rolx_core::stamp::{StampState, Status};
use rolx_core::week::{DAILY_TARGET_SECS, WeekSummary, fetch_week, guidance, summarize};

use crate::settings::Settings;
use crate::shared;

/// One color per activity, stable between runs. Unassigned entries are gray.
fn color(activity_id: i32) -> String {
    if activity_id == 0 {
        return "hsl(220, 8%, 55%)".into();
    }
    format!(
        "hsl({}, 55%, 52%)",
        u64::from(activity_id.unsigned_abs()) * 47 % 360
    )
}

/// Top of the chart scale: whole hours above everything shown, but at least a full day.
fn chart_max_secs(summary: &WeekSummary) -> i64 {
    let largest = summary
        .days
        .iter()
        .map(|d| d.booked_secs.max(d.nominal_secs))
        .max()
        .unwrap_or(0)
        .max(DAILY_TARGET_SECS);
    (largest + 3599) / 3600 * 3600
}

fn percent(part: i64, whole: i64) -> f64 {
    if whole <= 0 {
        0.0
    } else {
        (part as f64 / whole as f64 * 100.0).clamp(0.0, 100.0)
    }
}

/// The selected week: booked against target with what is left, and an optional chart.
#[component]
pub fn WeekPanel(
    mut day: Signal<NaiveDate>,
    refresh: Signal<u32>,
    state: Signal<StampState>,
) -> Element {
    let mut show_chart = use_signal(|| Settings::load().show_chart);
    let mut now = use_signal(|| Local::now().naive_local());

    // Minute-level is enough: guidance talks about hours.
    use_future(move || async move {
        loop {
            tokio::time::sleep(StdDuration::from_secs(20)).await;
            now.set(Local::now().naive_local());
        }
    });

    let week = use_resource(move || async move {
        refresh();
        let day = day();
        let client = shared::client().await?;
        fetch_week(&client, day).await
    });

    let balance = use_resource(move || async move {
        refresh();
        let client = shared::client().await?;
        fetch_balance(&client, Local::now().date_naive()).await
    });

    let toggle = move |_| {
        let next = !show_chart();
        show_chart.set(next);
        Settings::update(|s| s.show_chart = next);
    };

    let body = match &*week.read() {
        None => rsx! { p { class: "muted", "Loading week…" } },
        Some(Err(e)) => rsx! { p { class: "muted", "Week overview unavailable: {e}" } },
        Some(Ok(w)) => {
            let summary = summarize(w, &state.read().pending);
            let credited = summary.booked_secs + summary.leave_secs;
            let filled = percent(credited, summary.target_secs);
            let left = if summary.remaining_secs >= 0 {
                format!("{} left", fmt_hm(summary.remaining_secs))
            } else {
                format!("{} over", fmt_hm(-summary.remaining_secs))
            };
            let pensum = summary
                .pensum_percent
                .map(|p| format!(" · pensum {p} %"))
                .unwrap_or_default();
            let range = match (summary.days.first(), summary.days.last()) {
                (Some(a), Some(b)) => {
                    format!("{} – {}", a.date.format("%d.%m."), b.date.format("%d.%m."))
                }
                _ => String::new(),
            };
            let scale = chart_max_secs(&summary);
            let days = summary.days.clone();
            let legend: Vec<_> = summary.activities.iter().take(8).cloned().collect();
            let selected = day();
            let over = summary.remaining_secs < 0;

            let now_dt = now();
            let (running_secs, working) = match state.read().status(now_dt) {
                Status::StampedIn {
                    worked, on_break, ..
                } => (worked.num_seconds(), !on_break),
                Status::StampedOut => (0, false),
            };
            let advice = guidance(
                &summary,
                now_dt.date(),
                i64::from(now_dt.time().num_seconds_from_midnight()),
                running_secs,
                working,
            );
            let today_line = advice.today.as_ref().map(|t| t.describe());
            let balance_line = advice.balance_text();
            let missing = advice.missing_days.clone();
            let balances = balance
                .read()
                .as_ref()
                .and_then(|b| b.as_ref().ok())
                .map(|b| (b.overtime_text(), b.overtime_secs < 0, b.vacation_text()));

            rsx! {
                div { class: "card-head",
                    div { class: "week-head",
                        strong { "Week {selected.iso_week().week()}" }
                        span { class: "muted", " {range}" }
                    }
                    button { class: "secondary small", onclick: toggle,
                        if show_chart() { "Hide chart" } else { "Show chart" }
                    }
                }
                div { class: "week-numbers",
                    span { class: "big", "{fmt_hm(credited)}" }
                    span { class: "muted", " of {fmt_hm(summary.target_secs)}{pensum}" }
                    span { class: if over { "left over" } else { "left" }, "{left}" }
                }
                div { class: "progress",
                    div { class: if over { "fill over" } else { "fill" }, style: "width: {filled}%" }
                }
                if let Some((flextime, owed, vacation)) = balances {
                    div { class: "balances", title: "Balance as of today",
                        div { class: "stat",
                            span { class: "stat-label", "Flextime" }
                            strong { class: if owed { "owed" } else { "" }, "{flextime}" }
                        }
                        div { class: "stat",
                            span { class: "stat-label", "Vacation" }
                            strong { "{vacation}" }
                        }
                    }
                }
                div { class: "guidance",
                    if let Some(line) = today_line {
                        p { class: "today", "{line}" }
                    }
                    if let Some(line) = balance_line {
                        p { class: "muted", "{line}" }
                    }
                    if !missing.is_empty() {
                        p { class: "warn",
                            "⚠ Nothing booked on "
                            for d in missing.iter().copied() {
                                button {
                                    key: "{d}",
                                    class: "link",
                                    onclick: move |_| day.set(d),
                                    "{d.format(\"%a %d.%m.\")}"
                                }
                            }
                        }
                    }
                }
                if show_chart() {
                    div { class: "chart",
                        for d in days {
                            div {
                                key: "{d.date}",
                                class: match (d.date == selected, missing.contains(&d.date)) {
                                    (true, true) => "col sel missing",
                                    (true, false) => "col sel",
                                    (false, true) => "col missing",
                                    (false, false) => "col",
                                },
                                div { class: "bars", title: "{d.date.format(\"%a %d.%m.\")}: {fmt_hm(d.booked_secs)}",
                                    for t in d.by_activity.iter() {
                                        div {
                                            key: "{t.activity_id}",
                                            class: "seg",
                                            style: "height: {percent(t.secs, scale)}%; background: {color(t.activity_id)}",
                                            title: "{t.number} {fmt_hm(t.secs)}",
                                        }
                                    }
                                    if d.nominal_secs > 0 {
                                        div {
                                            class: "target",
                                            style: "bottom: {percent(d.nominal_secs, scale)}%",
                                            title: "Target {fmt_hm(d.nominal_secs)}",
                                        }
                                    }
                                }
                                div { class: "lbl", "{d.date.format(\"%a\")}" }
                                div { class: "val", if d.on_leave { "leave" } else { "{fmt_hm(d.booked_secs)}" } }
                            }
                        }
                    }
                    div { class: "legend",
                        for t in legend {
                            span { key: "{t.activity_id}", class: "legend-item",
                                span { class: "dot", style: "background: {color(t.activity_id)}" }
                                if t.activity_id == 0 {
                                    "no activity {fmt_hm(t.secs)}"
                                } else {
                                    "{t.number} {fmt_hm(t.secs)}"
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "card week", {body} }
    }
}
