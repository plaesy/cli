//! Cron expressions for scheduled jobs: `minute hour day-of-month month day-of-week`.
//!
//! Each field accepts `*`, a value, a range `a-b`, a list `a,b` (items may be ranges or steps),
//! and a step `*/n` or `a-b/n`. A bare `a/n` runs from `a` to the field maximum. Day-of-week is
//! 0-6 with Sunday as 0 or 7. Names (`MON`, `JAN`) and `@daily`-style macros are not supported.
//!
//! Day rule, as in Vixie cron: when both day-of-month and day-of-week are restricted (neither
//! starts with `*`), a day matches if either one matches. Otherwise both must match.
//!
//! All times are UTC, the clock the rest of the schedule uses. Everything here is pure
//! computation: no I/O and no network.

use crate::memory::error::{MemoryError, Result};
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, NaiveDateTime, Timelike, Utc};

/// How far `next_after` searches before giving up. Covers leap days (`0 0 29 2 *`) with room.
const HORIZON_DAYS: i64 = 366 * 5;

/// A parsed cron expression. Each field is a bitmask: bit `i` is set when value `i` is allowed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CronSpec {
    /// Minutes, 0-59.
    minutes: u64,
    /// Hours, 0-23.
    hours: u64,
    /// Days of the month, 1-31.
    days: u64,
    /// Months, 1-12.
    months: u64,
    /// Weekdays, 0-6 with Sunday as 0. A `7` in the expression is folded into 0.
    weekdays: u64,
    /// The day-of-month field starts with `*`.
    days_any: bool,
    /// The day-of-week field starts with `*`.
    weekdays_any: bool,
}

impl CronSpec {
    /// Parse a five-field expression. Fields are separated by whitespace.
    ///
    /// # Errors
    ///
    /// - `ConfigError` when there are not exactly five fields, a value is outside its range,
    ///   a number is malformed, a step is zero, or a range runs backwards
    pub fn parse(s: &str) -> Result<Self> {
        let fields: Vec<&str> = s.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(bad_expr(
                s,
                "expected 5 fields: minute hour day-of-month month day-of-week",
            ));
        }
        let (minutes, _) = parse_field(fields[0], 0, 59, "minute")?;
        let (hours, _) = parse_field(fields[1], 0, 23, "hour")?;
        let (days, days_any) = parse_field(fields[2], 1, 31, "day-of-month")?;
        let (months, _) = parse_field(fields[3], 1, 12, "month")?;
        let (weekdays, weekdays_any) = parse_field(fields[4], 0, 7, "day-of-week")?;
        Ok(Self {
            minutes,
            hours,
            days,
            months,
            weekdays: fold_sunday(weekdays),
            days_any,
            weekdays_any,
        })
    }

    /// Whether `t` falls on an occurrence, to the minute. Seconds are ignored.
    pub fn matches(&self, t: DateTime<Utc>) -> bool {
        let t = t.naive_utc();
        self.months & (1u64 << t.month()) != 0
            && self.day_matches(t.date())
            && self.hours & (1u64 << t.hour()) != 0
            && self.minutes & (1u64 << t.minute()) != 0
    }

    /// The first occurrence strictly after `after`, at a whole minute. `None` when no occurrence
    /// falls within the search horizon (about five years), e.g. `0 0 30 2 *`.
    pub fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let start = floor_minute(after.naive_utc())?;
        let limit = start.checked_add_signed(Duration::days(HORIZON_DAYS))?;
        let mut t = start.checked_add_signed(Duration::minutes(1))?;
        while t <= limit {
            if self.months & (1u64 << t.month()) == 0 {
                t = first_of_next_month(t.date())?.and_hms_opt(0, 0, 0)?;
            } else if !self.day_matches(t.date()) {
                t = t.date().succ_opt()?.and_hms_opt(0, 0, 0)?;
            } else if self.hours & (1u64 << t.hour()) == 0 {
                t = floor_hour(t)?.checked_add_signed(Duration::hours(1))?;
            } else if self.minutes & (1u64 << t.minute()) == 0 {
                t = t.checked_add_signed(Duration::minutes(1))?;
            } else {
                return Some(DateTime::from_naive_utc_and_offset(t, Utc));
            }
        }
        None
    }

    fn day_matches(&self, d: NaiveDate) -> bool {
        let dom = self.days & (1u64 << d.day()) != 0;
        let dow = self.weekdays & (1u64 << d.weekday().num_days_from_sunday()) != 0;
        if self.days_any || self.weekdays_any {
            dom && dow
        } else {
            dom || dow
        }
    }
}

/// Whether a cron job is due at `now`. A job that never ran is due. Otherwise it is due when an
/// occurrence falls after `last_run` and no later than `now`.
pub fn cron_is_due(spec: &CronSpec, last_run: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    match last_run {
        None => true,
        Some(t) => spec.next_after(t).is_some_and(|next| next <= now),
    }
}

fn bad_expr(expr: &str, why: &str) -> MemoryError {
    MemoryError::ConfigError(format!("bad cron expression '{expr}': {why}"))
}

fn bad_field(text: &str, name: &str, why: &str) -> MemoryError {
    MemoryError::ConfigError(format!("bad cron {name} field '{text}': {why}"))
}

/// Parse one field into a bitmask, returning the mask and whether the field starts with `*`.
fn parse_field(text: &str, min: u32, max: u32, name: &str) -> Result<(u64, bool)> {
    let mut mask = 0u64;
    for item in text.split(',') {
        let has_step = item.contains('/');
        let (range, step) = match item.split_once('/') {
            Some((range, step)) => (range, parse_num(step, text, name)?),
            None => (item, 1),
        };
        if step == 0 {
            return Err(bad_field(text, name, "step must be at least 1"));
        }
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (parse_num(a, text, name)?, parse_num(b, text, name)?)
        } else {
            let v = parse_num(range, text, name)?;
            (v, if has_step { max } else { v })
        };
        if lo < min || hi > max {
            return Err(bad_field(
                text,
                name,
                &format!("values must be between {min} and {max}"),
            ));
        }
        if lo > hi {
            return Err(bad_field(text, name, "range runs backwards"));
        }
        for v in (lo..=hi).step_by(step as usize) {
            mask |= 1u64 << v;
        }
    }
    Ok((mask, text.starts_with('*')))
}

/// Digits only: rejects signs, empty strings and anything `u32` cannot hold.
fn parse_num(s: &str, field: &str, name: &str) -> Result<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad_field(field, name, &format!("'{s}' is not a number")));
    }
    s.parse::<u32>()
        .map_err(|_| bad_field(field, name, &format!("'{s}' is too large")))
}

/// Sunday written as 7 becomes 0.
fn fold_sunday(weekdays: u64) -> u64 {
    let mut w = weekdays & 0x7f;
    if weekdays & (1u64 << 7) != 0 {
        w |= 1;
    }
    w
}

fn floor_minute(dt: NaiveDateTime) -> Option<NaiveDateTime> {
    dt.date().and_hms_opt(dt.hour(), dt.minute(), 0)
}

fn floor_hour(dt: NaiveDateTime) -> Option<NaiveDateTime> {
    dt.date().and_hms_opt(dt.hour(), 0, 0)
}

fn first_of_next_month(d: NaiveDate) -> Option<NaiveDate> {
    d.with_day(1)?.checked_add_months(Months::new(1))
}

#[cfg(test)]
#[path = "cron_test.rs"]
mod cron_test;
