use chrono::{DateTime, Utc};

/// The current time, also in the browser (chrono's `wasmbind` feature reads the JS clock there).
pub fn now() -> DateTime<Utc> {
    Utc::now()
}
