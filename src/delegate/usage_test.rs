//! Unit tests for `tests` split out of `usage.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use chrono::TimeZone;

use super::*;

fn record(provider: &str, input: Option<i64>, output: Option<i64>, at: &str) -> Record {
    Record {
        at: at.into(),
        provider: provider.into(),
        chain: String::new(),
        billable: true,
        input_tokens: input,
        output_tokens: output,
        wall_ms: 10,
    }
}

#[test]
fn unknown_counts_are_not_summed_as_zero() {
    let recs = [
        record("p", Some(100), Some(20), "2026-10-29T01:00:00Z"),
        record("p", None, None, "2026-10-29T02:00:00Z"),
    ];
    let (by_provider, _) = summarize(&recs);
    let totals = &by_provider["p"];
    assert_eq!(totals.calls, 2);
    assert_eq!(totals.input_tokens, 100);
    assert_eq!(totals.unknown_tokens, 1);
}

#[test]
fn budget_counts_only_todays_tokens_for_that_provider() {
    let now = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
    let recs = [
        record("p", Some(400), Some(400), "2026-10-29T03:00:00Z"),
        record("p", Some(9000), Some(9000), "2026-10-28T03:00:00Z"),
        record("other", Some(9000), Some(9000), "2026-10-29T03:00:00Z"),
    ];
    assert_eq!(tokens_on(&recs, "p", now), 800);
}

#[test]
fn a_billable_call_over_budget_is_refused_before_sending() {
    let now = Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap();
    let entry = DelegateProviderConfig {
        name: "p".into(),
        billable: true,
        daily_token_budget: 100,
        ..Default::default()
    };
    let req = Request {
        prompt: "x".repeat(40),
        max_output_tokens: 200,
        ..Request::default()
    };
    let err = check_budget(&entry, &req, &[], now).unwrap_err();
    assert!(err.message.contains("would be exceeded"));
}

#[test]
fn append_then_read_round_trips_and_skips_damaged_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join(USAGE_FILE);
    append(
        &path,
        &record("p", Some(1), Some(2), "2026-10-29T00:00:00Z"),
    )
    .unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"not json\n")
        .unwrap();
    append(&path, &record("q", None, None, "2026-10-29T00:00:01Z")).unwrap();
    let back = read(&path).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[1].provider, "q");
}
