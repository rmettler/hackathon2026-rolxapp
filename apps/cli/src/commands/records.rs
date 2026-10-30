use anyhow::anyhow;
use chrono::{Duration, NaiveDate};
use rolx_core::api::work_record;
use rolx_core::client::RolxClient;
use rolx_core::date_range::DateRange;

fn fmt_duration(d: Duration) -> String {
    format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60)
}

pub async fn run(client: &RolxClient, date: Option<String>) -> anyhow::Result<()> {
    let day = match date {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map_err(|_| anyhow!("invalid date '{s}', expected yyyy-MM-dd"))?,
        None => chrono::Local::now().date_naive(),
    };

    let records = work_record::get_range(
        &client.http,
        &client.server_url,
        &client.session,
        DateRange::day(day),
    )
    .await?;

    let entries: Vec<_> = records.into_iter().flat_map(|r| r.entries).collect();

    if entries.is_empty() {
        println!("No entries for {day}.");
        return Ok(());
    }

    for entry in &entries {
        let comment = entry.comment.as_deref().unwrap_or("");
        println!(
            "{:<20} {}  {}",
            entry.full_activity_number,
            fmt_duration(entry.duration),
            comment
        );
    }

    let total: Duration = entries.iter().map(|e| e.duration).sum();
    println!("{:<20} {}  total", "", fmt_duration(total));

    Ok(())
}
