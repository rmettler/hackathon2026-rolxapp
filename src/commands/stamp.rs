use anyhow::{anyhow, bail};
use chrono::{Local, NaiveDateTime};
use rolx_core::activities;
use rolx_core::dayplan::{fmt_clock, fmt_hm, parse_clock};
use rolx_core::records::fmt_duration;
use rolx_core::stamp::{ActivityRef, StampState, Status, most_recent_time};

use super::pick::{confirm, load_activities, resolve_activity};
use crate::cli::StampCommand;

/// `HH:MM` as the latest moment in the past with that time of day.
fn resolve_past_time(text: &str, now: NaiveDateTime) -> anyhow::Result<NaiveDateTime> {
    let secs =
        parse_clock(text).ok_or_else(|| anyhow!("--at must be a time like 17:30, got '{text}'"))?;
    Ok(most_recent_time(secs, now))
}

/// The activity for `words`, or the most recently used one for `--last`; `None` if neither is given.
fn choose(words: &[String], last: bool, state: &StampState) -> anyhow::Result<Option<ActivityRef>> {
    if last {
        let all = load_activities()?;
        return match activities::last_used(&all, &state.usage) {
            Some(a) => Ok(Some(a.to_ref())),
            None => bail!("no activity used yet, name one instead"),
        };
    }
    if words.is_empty() {
        return Ok(None);
    }
    Ok(Some(resolve_activity(words, &state.usage)?.to_ref()))
}

/// Refuses to silently turn a forgotten session into one huge entry.
fn refuse_stale(state: &StampState, now: NaiveDateTime) -> anyhow::Result<()> {
    if let Some(stale) = state.stale(now) {
        bail!(
            "the running session started {} ({} ago); end it at the right time with \
             `rolxapp stamp out --at HH:MM` or drop it with `rolxapp stamp cancel`",
            stale.since.format("%a %H:%M"),
            fmt_duration(stale.elapsed)
        );
    }
    Ok(())
}

fn on_activity(activity: &Option<ActivityRef>) -> String {
    match activity {
        Some(a) => format!(" on {}", a.full_number),
        None => " (no activity yet, set one with `stamp assign`)".to_string(),
    }
}

fn print_queue(state: &StampState) {
    if state.pending.is_empty() {
        return;
    }
    let unassigned = state.unassigned_count();
    let note = if unassigned > 0 {
        format!(" ({unassigned} without activity)")
    } else {
        String::new()
    };
    println!(
        "{} {} waiting for upload{note} — run `rolxapp sync`",
        state.pending.len(),
        if state.pending.len() == 1 {
            "entry"
        } else {
            "entries"
        }
    );
}

pub fn run(command: StampCommand) -> anyhow::Result<()> {
    let now = Local::now().naive_local();
    let time = now.format("%H:%M");
    let mut state = StampState::load()?;

    match command {
        StampCommand::In {
            activity,
            last,
            comment,
        } => {
            refuse_stale(&state, now)?;
            let activity = choose(&activity, last, &state)?;
            let label = on_activity(&activity);
            if state.stamp_in(now, activity, comment) > 0 {
                println!("Closed the previous session.");
            }
            println!("Stamped in at {time}{label}");
        }
        StampCommand::Out { comment, at } => {
            if at.is_none() {
                refuse_stale(&state, now)?;
            }
            if comment.is_some() {
                state.set_open_comment(comment)?;
            }
            match at {
                Some(text) => {
                    let end = resolve_past_time(&text, now)?;
                    state.stamp_out_at(end, now)?;
                    println!("Stamped out at {}", end.format("%a %H:%M"));
                }
                None => {
                    state.stamp_out(now)?;
                    println!("Stamped out at {time}");
                }
            }
        }
        StampCommand::Cancel => {
            if !state.discard_open() {
                bail!("not stamped in");
            }
            println!("Discarded the running session");
        }
        StampCommand::Break => {
            state.start_break(now)?;
            println!("Break started at {time}");
        }
        StampCommand::Resume => {
            state.end_break(now)?;
            println!("Break ended at {time}");
        }
        StampCommand::Switch {
            activity,
            last,
            comment,
        } => {
            refuse_stale(&state, now)?;
            let activity = choose(&activity, last, &state)?;
            let label = on_activity(&activity);
            state.switch(now, activity, comment)?;
            println!("Switched at {time}{label}");
        }
        StampCommand::Assign { activity } => {
            let activity = resolve_activity(&activity, &state.usage)?;
            let changed = state.assign_unassigned(&activity.to_ref(), now);
            println!("Assigned {} to {changed} item(s)", activity.full_number);
        }
        StampCommand::Queue => {
            if state.pending.is_empty() {
                println!("Nothing is waiting for upload");
            }
            for (i, e) in state.pending.iter().enumerate() {
                let end = i64::from(e.begin) + e.duration + i64::from(e.pause);
                println!(
                    "{:>3}  {}  {}-{}  {:>6}  {:<14} {}",
                    i + 1,
                    e.date,
                    fmt_clock(i64::from(e.begin)),
                    fmt_clock(end),
                    fmt_hm(e.duration),
                    e.activity.as_ref().map_or("(none yet)", |a| &a.full_number),
                    e.comment.as_deref().unwrap_or("")
                );
            }
            return Ok(());
        }
        StampCommand::Discard { number, yes } => {
            let Some(entry) = number.checked_sub(1).and_then(|i| state.pending.get(i)) else {
                bail!("there is no queued entry {number}, see `rolxapp stamp queue`");
            };
            let entry = entry.clone();
            let question = format!(
                "Discard {} {}-{} ({})?",
                entry.date,
                fmt_clock(i64::from(entry.begin)),
                fmt_clock(i64::from(entry.begin) + entry.duration + i64::from(entry.pause)),
                fmt_hm(entry.duration)
            );
            if !yes && !confirm(&question)? {
                println!("Kept");
                return Ok(());
            }
            state.remove_pending(&entry);
            println!("Discarded");
        }
        StampCommand::Status => {
            match state.status(now) {
                Status::StampedOut => println!("Stamped out"),
                Status::StampedIn {
                    since,
                    activity,
                    on_break,
                    worked,
                } => println!(
                    "Stamped in since {}{} — worked {}{}",
                    since.format("%H:%M"),
                    on_activity(&activity),
                    fmt_duration(worked),
                    if on_break { " (on break)" } else { "" }
                ),
            }
            print_queue(&state);
            if let Some(stale) = state.stale(now) {
                println!(
                    "⚠ This session has been running for {} — forgot to stamp out? \
                     Use `rolxapp stamp out --at HH:MM` or `rolxapp stamp cancel`",
                    fmt_duration(stale.elapsed)
                );
            }
            return Ok(());
        }
    }

    state.save()?;
    print_queue(&state);
    Ok(())
}
