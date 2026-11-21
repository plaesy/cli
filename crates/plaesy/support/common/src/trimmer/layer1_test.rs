//! Unit tests for `tests` split out of `layer1.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn repeats_collapse_and_long_output_keeps_head_and_tail() {
    assert_eq!(
        compress_lines(&strings(&["a", "a", "a", "b"]), 10, true, ""),
        ["a (x3)", "b"]
    );
    let long: Vec<String> = (0..10).map(|i| i.to_string()).collect();
    let out = compress_lines(&long, 4, false, "");
    assert_eq!(
        out,
        ["0", "1", "... 6 lines omitted ...", "8", "9"].map(String::from)
    );
}

#[test]
fn test_results_keep_failures_and_the_summary() {
    let out = compress_lines(
        &strings(&["ok a", "FAIL b", "5 passed"]),
        10,
        true,
        "test-results",
    );
    assert_eq!(out, ["FAIL b", "5 passed"]);
    let none = compress_lines(&strings(&["ok a"]), 10, true, "test-results");
    assert!(none[0].contains("all tests passed"));
}

#[test]
fn the_longest_matching_rule_wins() {
    let rules = Rules::default();
    assert_eq!(rule_for(&rules, "cargo test --all").max_lines, 40);
    assert_eq!(rule_for(&rules, "make").max_lines, DEFAULT_MAX_LINES);
}

#[test]
fn a_failing_command_keeps_its_output_and_reports_the_failure() {
    let rules = Rules::default();
    let args = if cfg!(windows) {
        strings(&["cmd", "/C", "echo boom & exit /B 3"])
    } else {
        strings(&["sh", "-c", "echo boom; exit 3"])
    };
    let outcome = run(&rules, None, &args).unwrap();
    assert!(outcome.output.contains("boom"));
    assert!(outcome.failure.is_some());
}
