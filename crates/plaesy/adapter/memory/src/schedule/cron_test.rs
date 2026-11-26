//! Unit tests for `cron.rs`. Kept as a separate file; included via `#[path]`.
//! All times are UTC. 2026-11-23 is a Monday.

use super::*;
use chrono::TimeZone;

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, 0)
        .single()
        .expect("valid test time")
}

#[test]
fn valid_expressions_parse() {
    for ok in [
        "* * * * *",
        "*/15 * * * *",
        "0 9 * * 1-5",
        "0,30 * * * *",
        "5-20/5 * * * *",
        "10/20 * * * *",
        "0 0 1 1 *",
        "  0   0 1 1 *  ",
    ] {
        assert!(CronSpec::parse(ok).is_ok(), "{ok} should parse");
    }
}

#[test]
fn bitmasks_follow_the_expression() {
    let s = CronSpec::parse("5-20/5 0,12 * * 7").expect("parse");
    assert_eq!(
        s.minutes,
        (1u64 << 5) | (1u64 << 10) | (1u64 << 15) | (1u64 << 20)
    );
    assert_eq!(s.hours, (1u64 << 0) | (1u64 << 12));
    // Day-of-month `*` covers 1-31.
    assert_eq!(s.days, ((1u64 << 32) - 1) & !1);
    // Sunday written as 7 is stored as 0.
    assert_eq!(s.weekdays, 1);
    assert!(s.days_any);
}

#[test]
fn boundary_values_are_inclusive_and_out_of_range_is_rejected() {
    for ok in ["0 0 1 1 0", "59 23 31 12 7", "59 23 31 12 6"] {
        assert!(CronSpec::parse(ok).is_ok(), "{ok} should parse");
    }
    for bad in [
        "",
        "* * * *",
        "* * * * * *",
        "60 * * * *",
        "* 24 * * *",
        "* * 0 * *",
        "* * 32 * *",
        "* * * 0 *",
        "* * * 13 *",
        "* * * * 8",
        "*/0 * * * *",
        "10/0 * * * *",
        "5-3 * * * *",
        "1-61 * * * *",
        "a * * * *",
        "1.5 * * * *",
        "-1 * * * *",
        "+1 * * * *",
        "1--2 * * * *",
        "1-2-3 * * * *",
        "/5 * * * *",
        "*/ * * * *",
        "1,,2 * * * *",
        "99999999999999 * * * *",
    ] {
        assert!(CronSpec::parse(bad).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn matches_checks_every_field() {
    let s = CronSpec::parse("30 8 * * 1").expect("parse");
    assert!(s.matches(at(2026, 11, 23, 8, 30)));
    assert!(!s.matches(at(2026, 11, 24, 8, 30)), "Tuesday");
    assert!(!s.matches(at(2026, 11, 23, 8, 31)));
    assert!(!s.matches(at(2026, 11, 23, 9, 30)));
}

#[test]
fn every_minute_drops_seconds_and_moves_one_minute() {
    let s = CronSpec::parse("* * * * *").expect("parse");
    let t = Utc
        .with_ymd_and_hms(2026, 11, 23, 10, 0, 59)
        .single()
        .expect("time");
    assert_eq!(s.next_after(t), Some(at(2026, 11, 23, 10, 1)));
}

#[test]
fn next_is_strictly_after_the_given_time() {
    let s = CronSpec::parse("0 9 * * *").expect("parse");
    assert_eq!(
        s.next_after(at(2026, 11, 23, 8, 59)),
        Some(at(2026, 11, 23, 9, 0))
    );
    assert_eq!(
        s.next_after(at(2026, 11, 23, 9, 0)),
        Some(at(2026, 11, 24, 9, 0))
    );
}

#[test]
fn steps_wrap_to_the_next_hour() {
    let s = CronSpec::parse("*/15 * * * *").expect("parse");
    assert_eq!(
        s.next_after(at(2026, 11, 23, 10, 0)),
        Some(at(2026, 11, 23, 10, 15))
    );
    assert_eq!(
        s.next_after(at(2026, 11, 23, 10, 45)),
        Some(at(2026, 11, 23, 11, 0))
    );
}

#[test]
fn ranges_with_steps_stop_at_the_range_end() {
    let s = CronSpec::parse("5-20/5 * * * *").expect("parse");
    assert_eq!(
        s.next_after(at(2026, 11, 23, 10, 20)),
        Some(at(2026, 11, 23, 11, 5))
    );
    assert_eq!(
        s.next_after(at(2026, 11, 23, 10, 4)),
        Some(at(2026, 11, 23, 10, 5))
    );
}

#[test]
fn hour_and_minute_lists_pick_the_earliest_later_slot() {
    let s = CronSpec::parse("30 8,17 * * *").expect("parse");
    assert_eq!(
        s.next_after(at(2026, 11, 23, 8, 30)),
        Some(at(2026, 11, 23, 17, 30))
    );
    assert_eq!(
        s.next_after(at(2026, 11, 23, 17, 30)),
        Some(at(2026, 11, 24, 8, 30))
    );
}

#[test]
fn weekday_filter_skips_to_the_next_matching_day() {
    let mon = CronSpec::parse("0 9 * * 1").expect("parse");
    assert_eq!(
        mon.next_after(at(2026, 11, 23, 9, 0)),
        Some(at(2026, 11, 30, 9, 0))
    );
    let sun = CronSpec::parse("0 9 * * 0").expect("parse");
    assert_eq!(
        sun.next_after(at(2026, 11, 23, 9, 0)),
        Some(at(2026, 11, 29, 9, 0))
    );
    let sun7 = CronSpec::parse("0 9 * * 7").expect("parse");
    assert_eq!(
        sun7.next_after(at(2026, 11, 23, 9, 0)),
        Some(at(2026, 11, 29, 9, 0))
    );
    let weekdays = CronSpec::parse("0 9 * * 1-5").expect("parse");
    // Friday 2026-11-27 -> next is Monday 2026-11-30.
    assert_eq!(
        weekdays.next_after(at(2026, 11, 27, 9, 0)),
        Some(at(2026, 11, 30, 9, 0))
    );
}

#[test]
fn restricted_day_of_month_and_weekday_use_either() {
    // The 13th or a Friday, at noon.
    let s = CronSpec::parse("0 12 13 * 5").expect("parse");
    assert_eq!(
        s.next_after(at(2026, 11, 23, 12, 0)),
        Some(at(2026, 11, 27, 12, 0))
    );
    assert_eq!(
        s.next_after(at(2026, 11, 27, 12, 0)),
        Some(at(2026, 12, 4, 12, 0))
    );
    assert_eq!(
        s.next_after(at(2026, 12, 4, 12, 0)),
        Some(at(2026, 12, 11, 12, 0))
    );
    // Sunday 2026-12-13 is the 13th, so it matches too.
    assert_eq!(
        s.next_after(at(2026, 12, 11, 12, 0)),
        Some(at(2026, 12, 13, 12, 0))
    );
}

#[test]
fn month_length_and_leap_day() {
    let s = CronSpec::parse("0 0 31 * *").expect("parse");
    // November has 30 days, so the next 31st is 2026-12-31.
    assert_eq!(
        s.next_after(at(2026, 11, 23, 0, 0)),
        Some(at(2026, 12, 31, 0, 0))
    );
    let leap = CronSpec::parse("0 0 29 2 *").expect("parse");
    assert_eq!(
        leap.next_after(at(2026, 3, 1, 0, 0)),
        Some(at(2028, 2, 29, 0, 0))
    );
}

#[test]
fn year_rollover_at_the_last_minute() {
    let s = CronSpec::parse("59 23 31 12 *").expect("parse");
    assert_eq!(
        s.next_after(at(2026, 12, 31, 23, 59)),
        Some(at(2027, 12, 31, 23, 59))
    );
}

#[test]
fn impossible_expression_returns_none_within_the_horizon() {
    let s = CronSpec::parse("0 0 30 2 *").expect("parse");
    assert_eq!(s.next_after(at(2026, 1, 1, 0, 0)), None);
}

#[test]
fn cron_due_after_an_occurrence_passes() {
    let s = CronSpec::parse("0 9 * * *").expect("parse");
    assert!(cron_is_due(&s, None, at(2026, 11, 23, 8, 0)));
    let ran = at(2026, 11, 23, 9, 0);
    assert!(!cron_is_due(&s, Some(ran), at(2026, 11, 23, 23, 0)));
    assert!(cron_is_due(&s, Some(ran), at(2026, 11, 24, 9, 0)));
    // Last run long ago: due once, since the next occurrence is already past.
    assert!(cron_is_due(
        &s,
        Some(at(2026, 11, 1, 9, 0)),
        at(2026, 11, 23, 10, 0)
    ));
}
