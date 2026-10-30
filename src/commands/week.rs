use chrono::Datelike;
use rolx_core::client::RolxClient;
use rolx_core::dayplan::fmt_hm;
use rolx_core::stamp::StampState;
use rolx_core::week::{DAILY_TARGET_SECS, fetch_week, summarize};

use super::records::parse_day;

const BAR_WIDTH: i64 = 20;
const BAR_FULL_SECS: i64 = 10 * 3600;

pub async fn run(client: &RolxClient, date: Option<String>) -> anyhow::Result<()> {
    let day = parse_day(date)?;
    let week = fetch_week(client, day).await?;
    let queued = match StampState::load() {
        Ok(state) => state.pending,
        Err(e) => {
            eprintln!("warning: queued entries not counted: {e}");
            Vec::new()
        }
    };
    let summary = summarize(&week, &queued);

    let (Some(first), Some(last)) = (summary.days.first(), summary.days.last()) else {
        anyhow::bail!("the server returned no days");
    };
    println!(
        "Week {} ({} – {})",
        day.iso_week().week(),
        first.date.format("%d.%m."),
        last.date.format("%d.%m.%Y")
    );

    for d in &summary.days {
        let filled = (d.booked_secs * BAR_WIDTH / BAR_FULL_SECS).clamp(0, BAR_WIDTH) as usize;
        println!(
            "{} {:>6} / {:>5}  {}{}{}",
            d.date.format("%a %d.%m."),
            fmt_hm(d.booked_secs),
            fmt_hm(d.nominal_secs),
            "█".repeat(filled),
            if d.on_leave { " (leave)" } else { "" },
            if d.date == day { "  ◀" } else { "" },
        );
    }

    let credited = summary.booked_secs + summary.leave_secs;
    let left = if summary.remaining_secs >= 0 {
        format!("{} left", fmt_hm(summary.remaining_secs))
    } else {
        format!("{} over", fmt_hm(-summary.remaining_secs))
    };
    println!(
        "Total {} of {}{} — {left}",
        fmt_hm(credited),
        fmt_hm(summary.target_secs),
        summary
            .pensum_percent
            .map(|p| format!(" (pensum {p} %)"))
            .unwrap_or_default(),
    );
    let on_day = summary
        .days
        .iter()
        .find(|d| d.date == day)
        .map_or(0, |d| d.booked_secs);
    println!(
        "Day target {}: {} booked",
        fmt_hm(DAILY_TARGET_SECS),
        fmt_hm(on_day)
    );
    Ok(())
}
