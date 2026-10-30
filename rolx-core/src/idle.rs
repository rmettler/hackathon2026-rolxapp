//! Noticing that the user was away from the computer while stamped in.
//!
//! The operating system tells how long ago the last keyboard or mouse input was; a frontend feeds
//! that into [`IdleTracker::observe`] regularly. Nothing here reads the system itself.

use chrono::{Duration, NaiveDateTime};

/// Time away that is long enough to ask about.
pub const DEFAULT_THRESHOLD_MINUTES: i64 = 10;

/// A finished absence: no input from `since` to `until`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Away {
    pub since: NaiveDateTime,
    pub until: NaiveDateTime,
}

impl Away {
    pub fn length(&self) -> Duration {
        self.until - self.since
    }
}

#[derive(Debug, Clone)]
pub struct IdleTracker {
    threshold: Duration,
    away_since: Option<NaiveDateTime>,
}

impl IdleTracker {
    pub fn new(threshold: Duration) -> Self {
        Self {
            threshold,
            away_since: None,
        }
    }

    /// Whether an absence has started and the user has not come back yet.
    pub fn is_away(&self) -> bool {
        self.away_since.is_some()
    }

    /// `idle` is the time since the last input. `tracking` tells whether it matters at all (stamped
    /// in and not on a break); otherwise any absence in progress is forgotten.
    /// Returns the absence once the user is back.
    pub fn observe(&mut self, now: NaiveDateTime, idle: Duration, tracking: bool) -> Option<Away> {
        if !tracking || self.threshold <= Duration::zero() {
            self.away_since = None;
            return None;
        }
        if idle >= self.threshold {
            self.away_since.get_or_insert(now - idle);
            return None;
        }
        let since = self.away_since.take()?;
        Some(Away {
            since,
            until: now - idle,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn at(h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 30)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap()
    }

    fn tracker() -> IdleTracker {
        IdleTracker::new(Duration::minutes(10))
    }

    #[test]
    fn short_pauses_are_ignored() {
        let mut t = tracker();
        assert_eq!(t.observe(at(10, 0, 0), Duration::minutes(9), true), None);
        assert!(!t.is_away());
        assert_eq!(t.observe(at(10, 1, 0), Duration::seconds(2), true), None);
    }

    #[test]
    fn an_absence_is_reported_when_the_user_returns() {
        let mut t = tracker();
        // last input was at 10:00:00, the user is noticed away at 10:10:30
        assert_eq!(
            t.observe(at(10, 10, 30), Duration::seconds(630), true),
            None
        );
        assert!(t.is_away());
        assert_eq!(t.observe(at(10, 30, 0), Duration::minutes(20), true), None);

        // input at 10:45:00, seen at 10:45:05
        let away = t
            .observe(at(10, 45, 5), Duration::seconds(5), true)
            .unwrap();
        assert_eq!(away.since, at(10, 0, 0));
        assert_eq!(away.until, at(10, 45, 0));
        assert_eq!(away.length(), Duration::minutes(45));
        assert!(!t.is_away());
        assert_eq!(t.observe(at(10, 45, 10), Duration::seconds(1), true), None);
    }

    #[test]
    fn nothing_is_reported_while_not_working() {
        let mut t = tracker();
        assert_eq!(t.observe(at(10, 30, 0), Duration::minutes(30), false), None);
        assert!(!t.is_away());
        assert_eq!(t.observe(at(10, 31, 0), Duration::seconds(1), false), None);
    }

    #[test]
    fn stopping_to_track_drops_the_absence() {
        let mut t = tracker();
        t.observe(at(10, 30, 0), Duration::minutes(30), true);
        assert!(t.is_away());
        t.observe(at(10, 31, 0), Duration::minutes(31), false);
        assert!(!t.is_away());
        assert_eq!(t.observe(at(10, 40, 0), Duration::seconds(1), true), None);
    }

    #[test]
    fn a_zero_threshold_switches_detection_off() {
        let mut t = IdleTracker::new(Duration::zero());
        assert_eq!(t.observe(at(10, 30, 0), Duration::hours(5), true), None);
        assert!(!t.is_away());
    }
}
