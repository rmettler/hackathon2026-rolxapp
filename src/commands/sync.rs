use rolx_core::client::RolxClient;
use rolx_core::config::AppConfig;
use rolx_core::sync;

pub async fn run(config: &AppConfig) -> anyhow::Result<()> {
    let client = RolxClient::new(config).await?;
    let report = sync::run_default(&client).await?;

    println!("Uploaded {} entries", report.uploaded);
    if report.unassigned > 0 {
        println!(
            "{} entries have no activity and stay local — use `rolxapp stamp assign <activity>`",
            report.unassigned
        );
    }
    for (day, reason) in &report.failed {
        eprintln!("{day}: not uploaded ({reason})");
    }
    if !report.failed.is_empty() {
        anyhow::bail!("{} day(s) could not be uploaded", report.failed.len());
    }
    Ok(())
}
