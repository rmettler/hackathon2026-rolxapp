use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use rolx_core::client::RolxClient;
use rolx_core::dayedit::load_day;
use rolx_core::dayplan::{DayEntry, Issue, analyze, fmt_clock, fmt_hm, sort_entries};
use rolx_core::stamp::StampState;

pub fn parse_day(date: Option<String>) -> anyhow::Result<NaiveDate> {
    match date {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map_err(|_| anyhow!("invalid date '{s}', expected yyyy-MM-dd")),
        None => Ok(chrono::Local::now().date_naive()),
    }
}

pub fn time_range(e: &DayEntry) -> String {
    match (e.begin, e.end()) {
        (Some(b), Some(end)) => format!("{}-{}", fmt_clock(i64::from(b)), fmt_clock(end)),
        _ => String::new(),
    }
}

pub fn entry_line(label: &str, e: &DayEntry) -> String {
    let mut duration = fmt_hm(e.duration);
    if let Some(pause) = e.pause.filter(|p| *p > 0) {
        duration.push_str(&format!(" +{} break", fmt_hm(i64::from(pause))));
    }
    format!(
        "{:<4} {:<11} {:<14} {:<16} {}",
        label,
        time_range(e),
        if e.full_activity_number.is_empty() {
            "(none yet)"
        } else {
            &e.full_activity_number
        },
        duration,
        e.comment.as_deref().unwrap_or("")
    )
}

pub fn describe_issue(issue: &Issue, label: &dyn Fn(usize) -> String) -> String {
    let text = issue.describe(label);
    if issue.is_overlap() {
        format!("⚠ {text}")
    } else {
        format!("· {text}")
    }
}

pub async fn run(client: &RolxClient, date: Option<String>) -> anyhow::Result<()> {
    let day = parse_day(date)?;
    let server = load_day(client, day).await?;
    let mut queued: Vec<DayEntry> = match StampState::load() {
        Ok(state) => state
            .pending
            .iter()
            .filter(|p| p.date == day)
            .map(DayEntry::from)
            .collect(),
        Err(e) => {
            eprintln!("warning: queued entries not shown: {e}");
            Vec::new()
        }
    };

    sort_entries(&mut queued);

    if server.is_empty() && queued.is_empty() {
        println!("No entries for {day}.");
        return Ok(());
    }

    for (i, entry) in server.iter().enumerate() {
        println!("{}", entry_line(&format!("#{}", i + 1), entry));
    }
    if !queued.is_empty() {
        println!("Waiting for upload:");
        for (i, entry) in queued.iter().enumerate() {
            println!("{}", entry_line(&format!("q{}", i + 1), entry));
        }
    }

    let total = Duration::seconds(server.iter().chain(&queued).map(|e| e.duration).sum());
    println!(
        "{:<4} {:<11} {:<14} {:<16}",
        "",
        "",
        "",
        rolx_core::records::fmt_duration(total).to_string() + "  total"
    );

    let all: Vec<DayEntry> = server.iter().chain(&queued).cloned().collect();
    let label = |i: usize| {
        if i < server.len() {
            format!("#{}", i + 1)
        } else {
            format!("q{}", i - server.len() + 1)
        }
    };
    for issue in analyze(&all) {
        println!("{}", describe_issue(&issue, &label));
    }
    Ok(())
}
