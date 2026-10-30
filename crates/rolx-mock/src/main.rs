use chrono::Duration;
use clap::Parser;
use rolx_mock::MockConfig;

#[derive(Parser)]
#[command(
    name = "rolx-mock",
    about = "Mock RolX API and Google OAuth server, for development and tests only"
)]
struct Args {
    /// Port to listen on, always on 127.0.0.1
    #[arg(long, default_value_t = 8090)]
    port: u16,

    /// Lifetime of issued bearer tokens in seconds; negative ones are expired on arrival
    #[arg(long, default_value_t = 86_400, allow_negative_numbers = true)]
    token_lifetime_secs: i64,

    /// Delay of the work record answers in milliseconds, to see loading states
    #[arg(long, default_value_t = 0)]
    work_record_delay_ms: u64,

    /// How many polls of the device grant stay pending, to look at the code screen
    #[arg(long, default_value_t = 2)]
    device_pending_polls: u32,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let config = MockConfig {
        token_lifetime: Duration::seconds(args.token_lifetime_secs),
        work_record_delay: std::time::Duration::from_millis(args.work_record_delay_ms),
        device_pending_polls: args.device_pending_polls,
        ..MockConfig::default()
    };
    let mock = rolx_mock::spawn_on_port(config, args.port).await?;
    println!("rolx-mock listening on {}", mock.base_url());
    mock.wait().await
}
