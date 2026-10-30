use anyhow::{anyhow, bail};
use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::dayedit::{load_day, save_day};
use rolx_core::dayplan::{EntryChange, Fix, apply_change, fix_new_issues, new_issues, parse_clock};
use rolx_core::stamp::StampState;

use super::pick::{confirm, resolve_activity};
use super::records::{describe_issue, entry_line, parse_day};
use crate::cli::{EntryCommand, FixKind};

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
            if change == EntryChange::default() {
                bail!("nothing to change, pass --activity, --comment, --begin or --end");
            }

            let client = RolxClient::new(config).await?;
            let base = load_day(&client, day).await?;
            let index = index_of(number, base.len(), day)?;

            let mut new = base.clone();
            apply_change(&mut new, index, &change)?;
            if let Some(kind) = fix {
                fix_new_issues(&mut new, &base, fix_for(kind))?;
            }

            let label = |i: usize| format!("#{}", i + 1);
            let created = new_issues(&base, &new);
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

            save_day(&client, day, &base, &new).await?;
            println!("Saved. See `rolxapp records {day}`");
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
            save_day(&client, day, &base, &new).await?;
            println!("Deleted #{number}");

            let label = |i: usize| format!("#{}", i + 1);
            for issue in new_issues(&base, &new) {
                println!("{}", describe_issue(&issue, &label));
            }
        }
    }
    Ok(())
}
