//! Typed wrappers around the RolX server's REST endpoints.

pub mod activity;
pub mod balance;
pub mod edit_lock;
pub mod records;
pub mod user_report;
pub mod year_info;

/// (De)serializes a `chrono::Duration` as the integer seconds the server uses for `TimeSpan`.
pub mod seconds {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i64(d.num_seconds())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::seconds(i64::deserialize(d)?))
    }
}

/// Formats a duration as `h:mm`, with a leading `-` if negative.
pub fn fmt_duration(d: chrono::Duration) -> String {
    let sign = if d < chrono::Duration::zero() {
        "-"
    } else {
        ""
    };
    let minutes = d.num_minutes().abs();
    format!("{sign}{}:{:02}", minutes / 60, minutes % 60)
}
