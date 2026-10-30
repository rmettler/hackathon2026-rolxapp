use tracing_subscriber::EnvFilter;

pub fn init(verbosity: u8) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match verbosity {
            0 => "warn",
            1 => "info",
            _ => "debug",
        };

        EnvFilter::new(level)
    });

    if let Err(err) = tracing_subscriber::fmt().with_env_filter(filter).try_init() {
        eprintln!("failed to initialize logging: {err}");
    }
}
