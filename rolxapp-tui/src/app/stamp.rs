use super::{App, Load, Msg};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, Timelike};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use rolx_core::api::records::{self, Record};

/// The part of a stamp that falls on one day, in seconds since midnight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StampPart {
    pub date: NaiveDate,
    pub begin: i32,
    pub end: i32,
}

/// What the stamp-out form needs: the days of the stamp and their current records.
pub struct StampSession {
    pub parts: Vec<StampPart>,
    pub records: Vec<Record>,
}

/// Whole minutes: the start is cut off at the minute, the end is rounded up, at least a minute.
pub fn stamp_range(
    started: DateTime<Local>,
    now: DateTime<Local>,
) -> (NaiveDateTime, NaiveDateTime) {
    let minute = |t: NaiveDateTime| {
        t.with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(t)
    };
    let from = minute(started.naive_local());
    let now = now.naive_local();
    let mut to = minute(now);
    if now > to {
        to += Duration::minutes(1);
    }
    if to <= from {
        to = from + Duration::minutes(1);
    }
    (from, to)
}

/// One part per day the stamp touches; a part that would be empty is left out.
pub fn split_stamp(from: NaiveDateTime, to: NaiveDateTime) -> Vec<StampPart> {
    let seconds = |t: NaiveDateTime| t.time().num_seconds_from_midnight() as i32;
    let mut parts = Vec::new();
    let mut date = from.date();
    while date <= to.date() {
        let begin = if date == from.date() {
            seconds(from)
        } else {
            0
        };
        let end = if date == to.date() {
            seconds(to)
        } else {
            86_400
        };
        if end > begin {
            parts.push(StampPart { date, begin, end });
        }
        date += Duration::days(1);
    }
    parts
}

impl App {
    /// Reads the running stamp left by an earlier run, if any.
    pub fn load_stamp(&mut self) {
        let Some(store) = &self.stamp_store else {
            return;
        };
        match store.load() {
            Ok(started) => self.stamp = started,
            Err(error) => {
                self.status = Some(format!(
                    "Can't read the stamp file {}: {error}",
                    store.path().display()
                ));
            }
        }
    }

    pub fn stamp_elapsed(&self, now: DateTime<Local>) -> Option<Duration> {
        self.stamp.map(|started| now - started)
    }

    pub(crate) fn toggle_stamp(&mut self) {
        let now = Local::now();
        match self.stamp {
            None => self.stamp_in(now),
            Some(_) => self.stamp_out(now),
        }
    }

    pub(crate) fn stamp_in(&mut self, now: DateTime<Local>) {
        self.stamp = Some(now);
        let saved = self.stamp_store.as_ref().map(|store| store.save(now));
        self.status = Some(match saved {
            Some(Err(error)) => format!("Stamped in, but the stamp can't be saved: {error}"),
            _ => format!("Stamped in at {}", now.format("%H:%M")),
        });
    }

    /// Loads the stamp's days, then opens the form that turns the stamp into entries.
    pub(crate) fn stamp_out(&mut self, now: DateTime<Local>) {
        let Some(started) = self.stamp else {
            return;
        };
        if self.searchable().is_empty() {
            self.status = Some("No activities available".into());
            return;
        }
        let (from, to) = stamp_range(started, now);
        let parts = split_stamp(from, to);
        let (Some(first), Some(last)) = (parts.first(), parts.last()) else {
            return;
        };
        let (first_day, after_last) = (first.date, last.date + Duration::days(1));
        self.spawn_load(Load::StampDays(parts.clone()), move |c| async move {
            let records = records::fetch_range(&c, first_day, after_last).await?;
            Ok(Msg::StampDays { parts, records })
        });
    }

    pub(crate) fn begin_discard(&mut self) {
        if self.stamp.is_some() {
            self.discard_prompt = true;
        } else {
            self.status = Some("No stamp is running".into());
        }
    }

    pub(crate) fn on_discard_key(&mut self, key: KeyEvent) {
        self.discard_prompt = false;
        if key.code == KeyCode::Char('y') {
            self.clear_stamp();
            self.status = Some("Stamp discarded".into());
        }
    }

    /// Forgets the running stamp, in memory and in its file.
    pub(crate) fn clear_stamp(&mut self) {
        self.stamp = None;
        self.stamp_session = None;
        if let Some(Err(error)) = self.stamp_store.as_ref().map(|store| store.clear()) {
            self.status = Some(format!("Can't remove the stamp file: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(day: u32, h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, day, h, m, s).unwrap()
    }

    fn naive(day: u32, h: u32, m: u32) -> NaiveDateTime {
        at(day, h, m, 0).naive_local()
    }

    #[test]
    fn the_start_is_cut_off_and_the_end_rounded_up_to_the_minute() {
        let (from, to) = stamp_range(at(30, 9, 12, 40), at(30, 10, 30, 5));

        assert_eq!(from, naive(30, 9, 12));
        assert_eq!(to, naive(30, 10, 31));
    }

    #[test]
    fn an_end_on_the_full_minute_stays() {
        let (_, to) = stamp_range(at(30, 9, 12, 40), at(30, 10, 30, 0));

        assert_eq!(to, naive(30, 10, 30));
    }

    #[test]
    fn a_stamp_is_at_least_one_minute_long() {
        let (from, to) = stamp_range(at(30, 9, 12, 0), at(30, 9, 12, 0));
        assert_eq!((from, to), (naive(30, 9, 12), naive(30, 9, 13)));

        let (from, to) = stamp_range(at(30, 9, 12, 10), at(30, 9, 12, 20));
        assert_eq!((from, to), (naive(30, 9, 12), naive(30, 9, 13)));
    }

    #[test]
    fn a_stamp_within_one_day_is_one_part() {
        let parts = split_stamp(naive(30, 17, 0), naive(30, 18, 30));

        assert_eq!(parts.len(), 1);
        assert_eq!((parts[0].begin, parts[0].end), (61_200, 66_600));
    }

    #[test]
    fn a_stamp_over_midnight_gets_one_part_per_day() {
        let parts = split_stamp(naive(26, 17, 0), naive(28, 9, 0));

        let spans: Vec<(u32, i32, i32)> = parts
            .iter()
            .map(|p| {
                (
                    p.date.format("%d").to_string().parse().unwrap(),
                    p.begin,
                    p.end,
                )
            })
            .collect();
        assert_eq!(
            spans,
            [(26, 61_200, 86_400), (27, 0, 86_400), (28, 0, 32_400)]
        );
    }

    #[test]
    fn ending_exactly_at_midnight_adds_no_empty_part() {
        let parts = split_stamp(naive(26, 17, 0), naive(27, 0, 0));

        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].end, 86_400);
    }
}
