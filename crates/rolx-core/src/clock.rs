use chrono::{DateTime, Local, NaiveDate, Utc};

/// The current time, also in the browser (chrono's `wasmbind` feature reads the JS clock there).
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// The current date in the user's time zone.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}
