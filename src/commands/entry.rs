use anyhow::{anyhow, bail};
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::dayedit::{load_day, previous_entries, save_day_tracked, undo_last};
use rolx_core::dayplan::{
    DayEntry, EntryChange, Fix, NewEntry, add_entry, apply_change, copyable, fix_new_issues,
    fmt_hm, new_issues, parse_clock, suggest_start,
};
use rolx_core::stamp::StampState;

use super::pick::{confirm, resolve_activity};
use super::records::{describe_issue, entry_line, parse_day};
use crate::cli::{EntryCommand, FixKind};

/// How far back `entry copy` looks for a day to repeat.
const COPY_LOOKBACK_DAYS: i64 = 7;

fn minutes_to_secs(flag: &str, minutes: u32) -> anyhow::Result<i32> {
    minutes
        .checked_mul(60)
        .and_then(|secs| i32::try_from(secs).ok())
        .ok_or_else(|| anyhow!("{flag} is too long"))
}

/// Applies `--fix` to the conflicts `new` has compared to `base`, and refuses to go on with
/// overlaps that remain unless `--force` was given. Gaps are only reported.
fn resolve_conflicts(
    base: &[DayEntry],
    new: &mut Vec<DayEntry>,
    fix: Option<FixKind>,
    force: bool,
) -> anyhow::Result<()> {
    if let Some(kind) = fix {
        fix_new_issues(new, base, fix_for(kind))?;
    }
    let label = |i: usize| format!("#{}", i + 1);
    let created = new_issues(base, new);
    let overlaps: Vec<_> = created.iter().filter(|i| i.is_overlap()).collect();
    if !overlaps.is_empty() && !force {
        for issue in &overlaps {
            eprintln!("{}", describe_issue(issue, &label));
        }
        bail!(
            "the change makes entries overlap; use --fix truncate|shift to resolve it, \
             or --force to save anyway"
        );
    }
    for issue in created.iter().filter(|i| !i.is_overlap()) {
        println!("{}", describe_issue(issue, &label));
    }
    Ok(())
}
fn fix_for(kind: FixKind) -> Fix {
    match kind {
        FixKind::Truncate => Fix::TruncateFirst,
        FixKind::Shift => Fix::ShiftSecond,
        FixKind::Break => Fix::FillAsBreak,
    }
}

fn parse_time(flag: &str, text: Option<String>) -> anyhow::Result<Option<i32>> {
    text.map(|t| {
        parse_clock(&t).ok_or_else(|| anyhow!("{flag} must be a time like 09:30, got '{t}'"))
    })
    .transpose()
}

fn index_of(number: usize, len: usize, day: impl std::fmt::Display) -> anyhow::Result<usize> {
    number
        .checked_sub(1)
        .filter(|i| *i < len)
        .ok_or_else(|| anyhow!("there is no entry {number} on {day}, see `rolxapp records {day}`"))
}

pub async fn run(config: &AppConfig, command: EntryCommand) -> anyhow::Result<()> {
    match command {
        EntryCommand::Edit {
            number,
            date,
            activity,
            comment,
            begin,
            end,
            pause,
            fix,
            force,
        } => {
            let day = parse_day(date)?;
            let mut change = EntryChange::default();
            if let Some(query) = activity {
                let usage = StampState::load()?.usage;
                let found = resolve_activity(&[query], &usage)?;
                change.activity = Some((found.id, found.full_number));
            }
            change.comment = comment.map(|c| Some(c).filter(|c| !c.trim().is_empty()));
            change.begin = parse_time("--begin", begin)?;
            change.end = parse_time("--end", end)?;
            change.pause = pause
                .map(|minutes| minutes_to_secs("--break", minutes))
                .transpose()?;
            if change == EntryChange::default() {
                bail!("nothing to change, pass --activity, --comment, --begin, --end or --break");
            }

            let client = RolxClient::new(config).await?;
            let base = load_day(&client, day).await?;
            let index = index_of(number, base.len(), day)?;

            let mut new = base.clone();
            apply_change(&mut new, index, &change)?;
            resolve_conflicts(&base, &mut new, fix, force)?;

            save_day_tracked(&client, day, &base, &new, &format!("Edited #{number}")).await?;
            println!("Saved. See `rolxapp records {day}`; `rolxapp entry undo` takes it back");
        }
        EntryCommand::Add {
            activity,
            begin,
            end,
            pause,
            comment,
            date,
            fix,
            force,
        } => {
            let day = parse_day(date)?;
            let usage = StampState::load()?.usage;
            let found = resolve_activity(&[activity], &usage)?;
            let client = RolxClient::new(config).await?;
            let base = load_day(&client, day).await?;

            let begin = match parse_time("--begin", begin)? {
                Some(begin) => begin,
                None => suggest_start(&base)
                    .ok_or_else(|| anyhow!("--begin is needed, nothing is booked on {day} yet"))?,
            };
            let end = parse_time("--end", Some(end))?.ok_or_else(|| anyhow!("--end is needed"))?;
            let entry = NewEntry {
                activity: (found.id, found.full_number.clone()),
                begin,
                end,
                pause: pause.map_or(Ok(0), |m| minutes_to_secs("--break", m))?,
                comment,
            };

            let mut new = base.clone();
            add_entry(&mut new, &entry)?;
            resolve_conflicts(&base, &mut new, fix, force)?;

            let what = format!("Added {} on {day}", found.full_number);
            save_day_tracked(&client, day, &base, &new, &what).await?;
            println!("Added. See `rolxapp records {day}`; `rolxapp entry undo` takes it back");
        }
        EntryCommand::Copy { from, date } => {
            let day = parse_day(date)?;
            let client = RolxClient::new(config).await?;
            let base = load_day(&client, day).await?;
            if !base.is_empty() {
                bail!("{day} already has entries, entries are only copied onto an empty day");
            }
            if StampState::load()?.pending.iter().any(|p| p.date == day) {
                bail!("{day} has entries waiting for upload, run `rolxapp sync` first");
            }

            let found = match from {
                Some(from) => {
                    let source = parse_day(Some(from))?;
                    if source == day {
                        bail!("cannot copy a day onto itself");
                    }
                    Some((source, copyable(&load_day(&client, source).await?)))
                }
                None => previous_entries(&client, day, COPY_LOOKBACK_DAYS).await?,
            };
            let Some((source, entries)) = found.filter(|(_, e)| !e.is_empty()) else {
                bail!("found nothing to copy");
            };

            for e in &entries {
                println!("{}", entry_line("+", e));
            }
            let total: i64 = entries.iter().map(|e| e.duration).sum();
            let noun = if entries.len() == 1 {
                "entry"
            } else {
                "entries"
            };
            let what = format!("Copied {} {noun} from {source}", entries.len());
            save_day_tracked(&client, day, &base, &entries, &what).await?;
            println!(
                "Copied {} {noun} ({}) from {source} to {day}; `rolxapp entry undo` takes it back",
                entries.len(),
                fmt_hm(total)
            );
        }
        EntryCommand::Undo => {
            let client = RolxClient::new(config).await?;
            let undone = undo_last(&client).await?;
            println!("Undone: {} ({})", undone.what, undone.day);
        }
        EntryCommand::Delete { number, date, yes } => {
            let day = parse_day(date)?;
            let client = RolxClient::new(config).await?;
            let base = load_day(&client, day).await?;
            let index = index_of(number, base.len(), day)?;

            let question = format!(
                "Delete {}?",
                entry_line(&format!("#{number}"), &base[index]).trim()
            );
            if !yes && !confirm(&question)? {
                println!("Kept");
                return Ok(());
            }

            let mut new = base.clone();
            new.remove(index);
            save_day_tracked(&client, day, &base, &new, &format!("Deleted #{number}")).await?;
            println!("Deleted #{number}; `rolxapp entry undo` takes it back");

            let label = |i: usize| format!("#{}", i + 1);
            for issue in new_issues(&base, &new) {
                println!("{}", describe_issue(&issue, &label));
            }
        }
    }
    Ok(())
}
