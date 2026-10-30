use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "rolxapp-webhost",
    about = "Serves the RolX web app and forwards /api to the RolX server"
)]
struct Args {
    /// The RolX server to forward /api to
    #[arg(long, env = "ROLX_SERVER_URL")]
    server: String,

    /// Directory with the web app's index.html; without it only /api and /config.json are served
    #[arg(long, env = "ROLX_BUNDLE_DIR")]
    bundle_dir: Option<PathBuf>,

    /// Address to listen on. Loopback by default: the app has no TLS of its own
    #[arg(long, env = "ROLX_LISTEN", default_value = "127.0.0.1:8080")]
    listen: SocketAddr,

    /// Google OAuth client id of the "Web application" type, for the sign-in button
    #[arg(long, env = "ROLX_GOOGLE_CLIENT_ID")]
    google_client_id: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    if !args.server.starts_with("https://") && !args.server.starts_with("http://") {
        return Err("--server must be an http or https address".into());
    }
    if let Some(dir) = args
        .bundle_dir
        .as_ref()
        .filter(|d| !d.join("index.html").is_file())
    {
        return Err(format!("no index.html in {}", dir.display()).into());
    }

    if args.bundle_dir.is_none() {
        println!("no --bundle-dir: serving only /api and /config.json");
    }

    let app = rolxapp_webhost::app(rolxapp_webhost::Config {
        server: args.server,
        bundle_dir: args.bundle_dir,
        google_client_id: args.google_client_id,
    })?;

    let listener = TcpListener::bind(args.listen).await?;
    println!(
        "rolxapp-webhost listening on http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Stops on Ctrl-C and on SIGTERM. A container's PID 1 has to do this itself: without a handler the
/// kernel ignores the signal, and `podman stop` would wait for its timeout.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = terminate.recv() => {}
                    _ = tokio::signal::ctrl_c() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
