//! Unit tests for `tests` split out of `usage.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use chrono::TimeZone;

use super::*;

/// Prices in micro-USD per million tokens: USD 3 in and USD 15 out.
const INPUT_PRICE: i64 = 3_000_000;
const OUTPUT_PRICE: i64 = 15_000_000;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 29, 12, 0, 0).unwrap()
}

fn priced(name: &str, billable: bool) -> DelegateProviderConfig {
    DelegateProviderConfig {
        name: name.into(),
        billable,
        input_micro_usd_per_mtok: INPUT_PRICE,
        output_micro_usd_per_mtok: OUTPUT_PRICE,
        ..Default::default()
    }
}

/// A priced record dated on the day of [`now`], so the daily budget counts it.
fn spent_today(entry: &DelegateProviderConfig, task: &str, input: u64, output: u64) -> Record {
    let mut rec = record_for(entry, "", task, Some(input), Some(output), 1);
    rec.at = "2026-10-29T03:00:00Z".into();
    rec
}

fn record(provider: &str, input: Option<i64>, output: Option<i64>, at: &str) -> Record {
    Record {
        at: at.into(),
        provider: provider.into(),
        chain: String::new(),
        task: String::new(),
        billable: true,
        input_tokens: input,
        output_tokens: output,
        cost_micro_usd: None,
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
    let recs = [
        record("p", Some(400), Some(400), "2026-10-29T03:00:00Z"),
        record("p", Some(9000), Some(9000), "2026-10-28T03:00:00Z"),
        record("other", Some(9000), Some(9000), "2026-10-29T03:00:00Z"),
    ];
    assert_eq!(tokens_on(&recs, "p", now()), 800);
}

#[test]
fn a_billable_call_over_budget_is_refused_before_sending() {
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
    let err = check_budget(&entry, &req, &[], now()).unwrap_err();
    assert!(err.message.contains("would be exceeded"));
}

#[test]
fn a_call_within_the_token_budget_is_allowed() {
    let entry = DelegateProviderConfig {
        name: "p".into(),
        billable: true,
        daily_token_budget: 1_000,
        ..Default::default()
    };
    let req = Request {
        prompt: "x".repeat(40),
        max_output_tokens: 200,
        ..Request::default()
    };
    assert!(check_budget(&entry, &req, &[], now()).is_ok());
}

#[test]
fn record_for_prices_the_call_and_carries_chain_and_task() {
    let rec = record_for(
        &priced("p", true),
        "chain-a",
        "task-a",
        Some(100_000),
        Some(100_000),
        7,
    );
    // 100_000 * 3_000_000 + 100_000 * 15_000_000 = 1_800_000_000_000, over 1_000_000 = 1_800_000.
    assert_eq!(rec.cost_micro_usd, Some(1_800_000));
    assert_eq!(rec.provider, "p");
    assert_eq!(rec.chain, "chain-a");
    assert_eq!(rec.task, "task-a");
    assert!(rec.billable);
}

#[test]
fn record_for_has_no_cost_without_a_price_or_a_token_count() {
    let unpriced = DelegateProviderConfig {
        name: "free".into(),
        ..Default::default()
    };
    assert_eq!(
        record_for(&unpriced, "", "", Some(5), Some(5), 1).cost_micro_usd,
        None
    );
    assert_eq!(
        record_for(&priced("p", true), "", "", None, Some(5), 1).cost_micro_usd,
        None
    );
}

#[test]
fn cost_is_summed_per_task_and_uncosted_calls_are_counted() {
    let entry = priced("p", true);
    let recs = [
        record_for(&entry, "", "task-a", Some(1_000_000), Some(0), 1),
        record_for(&entry, "", "task-a", Some(100_000), Some(100_000), 1),
        record_for(&entry, "", "task-b", None, None, 1),
        record_for(
            &DelegateProviderConfig {
                name: "free".into(),
                ..Default::default()
            },
            "",
            "task-b",
            Some(5),
            Some(5),
            1,
        ),
    ];
    let by_task = summarize_by_task(&recs);

    let a = &by_task["task-a"];
    assert_eq!(a.calls, 2);
    // 3_000_000 for the first call plus 1_800_000 for the second.
    assert_eq!(a.cost_micro_usd, 4_800_000);
    assert_eq!(a.uncosted_calls, 0);

    let b = &by_task["task-b"];
    assert_eq!(b.calls, 2);
    assert_eq!(b.cost_micro_usd, 0);
    assert_eq!(b.uncosted_calls, 2);
    assert_eq!(b.unknown_tokens, 1);
}

#[test]
fn a_direct_call_without_a_task_is_grouped_under_the_empty_name() {
    let recs = [record_for(&priced("p", true), "", "", Some(1), Some(1), 1)];
    let by_task = summarize_by_task(&recs);
    assert!(by_task.contains_key(""));
}

#[test]
fn cost_budget_refuses_a_call_that_would_pass_the_limit() {
    // Spent today: 3_000_000. The call estimates 10 input tokens and 200 output tokens,
    // which is 30 + 3_000 = 3_030 micro-USD. 3_003_030 is over a limit of 3_000_100.
    let mut entry = priced("p", true);
    entry.daily_cost_budget_micro_usd = 3_000_100;
    let recs = [spent_today(&priced("p", true), "task-a", 1_000_000, 0)];
    let req = Request {
        prompt: "x".repeat(40),
        max_output_tokens: 200,
        ..Request::default()
    };
    let err = check_budget(&entry, &req, &recs, now()).unwrap_err();
    assert!(err.message.contains("cost budget"), "{}", err.message);
    assert!(err.message.contains("would be exceeded"), "{}", err.message);
}

#[test]
fn cost_budget_allows_a_call_within_the_limit() {
    let mut entry = priced("p", true);
    entry.daily_cost_budget_micro_usd = 5_000_000;
    let recs = [spent_today(&priced("p", true), "task-a", 1_000_000, 0)];
    let req = Request {
        prompt: "x".repeat(40),
        max_output_tokens: 200,
        ..Request::default()
    };
    assert!(check_budget(&entry, &req, &recs, now()).is_ok());
}

#[test]
fn cost_budget_ignores_records_from_other_days() {
    let mut entry = priced("p", true);
    // Counting yesterday's 3_000_000 would refuse (3_003_030 > 3_002_000), so this proves the day filter.
    entry.daily_cost_budget_micro_usd = 3_002_000;
    let mut yesterday = record_for(
        &priced("p", true),
        "",
        "task-a",
        Some(1_000_000),
        Some(0),
        1,
    );
    yesterday.at = "2026-10-28T03:00:00Z".into();
    let req = Request {
        prompt: "x".repeat(40),
        max_output_tokens: 200,
        ..Request::default()
    };
    assert!(check_budget(&entry, &req, &[yesterday], now()).is_ok());
}

#[test]
fn cost_budget_without_a_price_is_refused_before_sending() {
    let entry = DelegateProviderConfig {
        name: "p".into(),
        billable: true,
        daily_cost_budget_micro_usd: 1_000,
        ..Default::default()
    };
    let err = check_budget(&entry, &Request::default(), &[], now()).unwrap_err();
    assert!(err.message.contains("no token price"), "{}", err.message);
}

#[test]
fn cost_budget_does_not_limit_a_non_billable_provider() {
    let mut entry = priced("p", false);
    entry.daily_cost_budget_micro_usd = 1;
    let req = Request {
        prompt: "x".repeat(4_000),
        max_output_tokens: 4_000,
        ..Request::default()
    };
    assert!(check_budget(&entry, &req, &[], now()).is_ok());
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

#[test]
fn a_priced_record_keeps_its_task_and_cost_through_the_log() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(USAGE_FILE);
    let rec = record_for(
        &priced("p", true),
        "",
        "task-a",
        Some(1_000_000),
        Some(0),
        1,
    );
    append(&path, &rec).unwrap();
    let back = read(&path).unwrap();
    assert_eq!(back[0].task, "task-a");
    assert_eq!(back[0].cost_micro_usd, Some(3_000_000));
}

#[test]
fn records_written_before_task_and_cost_existed_still_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(USAGE_FILE);
    fs::write(
        &path,
        "{\"at\":\"2026-10-29T00:00:00Z\",\"provider\":\"p\",\"billable\":true,\"input_tokens\":1,\"output_tokens\":2,\"wall_ms\":3}\n",
    )
    .unwrap();
    let back = read(&path).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].task, "");
    assert_eq!(back[0].cost_micro_usd, None);
}
