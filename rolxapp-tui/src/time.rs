use chrono::Duration;
use rolx_core::api::records::RecordEntry;

/// Parses `h:mm` or `hhmm` into seconds since midnight.
pub(crate) fn parse_clock(s: &str) -> Option<i32> {
    let s = s.trim();
    let (h, m): (i32, i32) = match s.split_once(':') {
        Some((h, m)) => (h.parse().ok()?, m.parse().ok()?),
        None if (3..=4).contains(&s.len()) => {
            let (h, m) = s.split_at(s.len() - 2);
            (h.parse().ok()?, m.parse().ok()?)
        }
        None => return None,
    };
    ((0..=24).contains(&h) && (0..60).contains(&m) && h * 60 + m <= 24 * 60)
        .then_some((h * 60 + m) * 60)
}

/// Formats seconds since midnight as `HH:MM`.
pub fn fmt_clock(secs: i64) -> String {
    format!("{:02}:{:02}", secs / 3600, secs % 3600 / 60)
}

/// End of an entry in seconds since midnight.
pub fn entry_end(begin: i32, entry: &RecordEntry) -> i64 {
    i64::from(begin) + i64::from(entry.pause.unwrap_or(0)) + entry.duration.num_seconds()
}

/// Parses `h:mm` or decimal hours.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some((h, m)) = s.split_once(':') {
        let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
        return ((h >= 0) && (0..60).contains(&m)).then(|| Duration::minutes(h * 60 + m));
    }
    let hours: f64 = s.parse().ok()?;
    (hours >= 0.0).then(|| Duration::minutes((hours * 60.0).round() as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clock_and_decimal_durations() {
        assert_eq!(parse_duration("1:30"), Some(Duration::minutes(90)));
        assert_eq!(parse_duration("0.5"), Some(Duration::minutes(30)));
        assert_eq!(parse_duration("8"), Some(Duration::hours(8)));
    }

    #[test]
    fn parses_clock_times() {
        assert_eq!(parse_clock("9:30"), Some(34_200));
        assert_eq!(parse_clock("0930"), Some(34_200));
        assert_eq!(parse_clock("930"), Some(34_200));
        assert_eq!(parse_clock("24:00"), Some(86_400));
        assert_eq!(parse_clock("25:00"), None);
        assert_eq!(parse_clock("9:75"), None);
        assert_eq!(parse_clock("9"), None);
    }

    #[test]
    fn rejects_invalid_durations() {
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("1:75"), None);
        assert_eq!(parse_duration("-1:30"), None);
        assert_eq!(parse_duration("abc"), None);
    }
}
