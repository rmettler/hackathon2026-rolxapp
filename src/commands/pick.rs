//! Prompts shared by the commands that need an activity or a confirmation.

use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};

use anyhow::bail;
use rolx_core::activities::{self, Activity, Group, ResolveError, resolve, search};
use rolx_core::stamp::Usage;

pub fn marker(group: Group) -> &'static str {
    match group {
        Group::Favourite => "★",
        Group::Recent => "•",
        Group::Other => " ",
    }
}

pub fn load_activities() -> anyhow::Result<Vec<Activity>> {
    let all = activities::load_cached()?;
    if all.is_empty() {
        bail!("no activity list cached yet — run `rolxapp sync` once while online");
    }
    Ok(all)
}

/// Finds the activity for the given words. If they match several and a person is at the terminal,
/// offers a numbered list instead of failing.
pub fn resolve_activity(
    words: &[String],
    usage: &BTreeMap<i32, Usage>,
) -> anyhow::Result<Activity> {
    let all = load_activities()?;
    let query = words.join(" ");
    match resolve(&all, &query) {
        Ok(found) => Ok(found.clone()),
        Err(ResolveError::Ambiguous { query, .. }) if io::stdin().is_terminal() => {
            pick(&all, usage, &query)
        }
        Err(e) => Err(e.into()),
    }
}

const PICK_LIMIT: usize = 15;

fn pick(all: &[Activity], usage: &BTreeMap<i32, Usage>, query: &str) -> anyhow::Result<Activity> {
    let hits = search(all, usage, query);
    let shown = &hits[..hits.len().min(PICK_LIMIT)];

    println!("'{query}' matches {} activities:", hits.len());
    for (i, hit) in shown.iter().enumerate() {
        println!(
            "{:>3}) {} {}",
            i + 1,
            marker(hit.group),
            hit.activity.label()
        );
    }
    if hits.len() > shown.len() {
        println!(
            "     … {} more, use more words to narrow it down",
            hits.len() - shown.len()
        );
    }

    let answer = ask(&format!("Pick 1-{} (Enter to cancel): ", shown.len()))?;
    match answer.trim().parse::<usize>() {
        Ok(n) if (1..=shown.len()).contains(&n) => Ok(shown[n - 1].activity.clone()),
        _ => bail!("cancelled"),
    }
}

fn ask(prompt: &str) -> anyhow::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line)
}

/// Asks for confirmation. Without a terminal there is nobody to ask, so `--yes` is required.
pub fn confirm(question: &str) -> anyhow::Result<bool> {
    if !io::stdin().is_terminal() {
        bail!("cannot ask for confirmation here, pass --yes");
    }
    Ok(matches!(
        ask(&format!("{question} [y/N] "))?
            .trim()
            .to_lowercase()
            .as_str(),
        "y" | "yes"
    ))
}
