use std::fmt;

use chrono::{Days, NaiveDate};

/// Half-open date range: includes `begin`, excludes `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub begin: NaiveDate,
    pub end: NaiveDate,
}

impl DateRange {
    /// The range covering only `date`.
    pub fn day(date: NaiveDate) -> Self {
        Self {
            begin: date,
            end: date + Days::new(1),
        }
    }
}

/// The form used in routes: `yyyy-MM-dd..yyyy-MM-dd`.
impl fmt::Display for DateRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}..{}",
            self.begin.format("%Y-%m-%d"),
            self.end.format("%Y-%m-%d")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn day_ends_on_the_next_day() {
        let range = DateRange::day(date(2026, 10, 30));
        assert_eq!(range.to_string(), "2026-10-30..2026-10-31");
    }

    #[test]
    fn day_crosses_month_and_year_ends() {
        assert_eq!(
            DateRange::day(date(2026, 12, 31)).to_string(),
            "2026-12-31..2027-01-01"
        );
    }

    #[test]
    fn day_handles_the_leap_day() {
        assert_eq!(
            DateRange::day(date(2028, 2, 28)).to_string(),
            "2028-02-28..2028-02-29"
        );
    }
}
