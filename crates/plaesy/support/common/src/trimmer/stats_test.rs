//! Unit tests for `tests` split out of `stats.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn estimates_round_up_and_savings_round_to_one_decimal() {
    assert_eq!(estimate_tokens("abcde"), 2);
    assert_eq!(pct_saved(3, 2), 33.3);
    assert_eq!(pct_saved(0, 0), 0.0);
}

#[test]
fn report_says_when_there_is_nothing_yet() {
    let dir = tempfile::tempdir().unwrap();
    assert!(report(&dir.path().join("none.json")).starts_with("No token stats yet"));
}

#[test]
fn records_accumulate_and_report_totals() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("token-stats.json");
    add_record(&path, "layer1", "git status", 100, 50).unwrap();
    add_record(&path, "layer1", "git status", 100, 50).unwrap();
    assert!(report(&path).contains("total: 200 -> 100 tokens (50.0% saved)"));
}
