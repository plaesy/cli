//! Unit tests for `tests` split out of `expected.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn outcome(expected: &str, output: &str) -> Outcome {
    judge(expected, output).outcome
}

#[test]
fn binary_expectations() {
    assert_eq!(outcome("pass", ""), Outcome::Met);
    assert_eq!(outcome("zero", "0"), Outcome::Met);
    assert_eq!(outcome("zero", "3"), Outcome::NotMet);
    assert_eq!(outcome("all", "5 of 5"), Outcome::Met);
    assert_eq!(outcome("all", "4 of 5"), Outcome::NotMet);
    assert_eq!(outcome("all pages", "pass"), Outcome::Met);
    assert_eq!(outcome("every route", "3 of 4"), Outcome::NotMet);
}

#[test]
fn numeric_comparisons_and_units() {
    assert_eq!(outcome(">= 9", "9"), Outcome::Met);
    assert_eq!(outcome(">= 9", "8"), Outcome::NotMet);
    assert_eq!(outcome("<= 2 s", "1.5 s"), Outcome::Met);
    assert_eq!(outcome("<= 2 s", "1500 ms"), Outcome::Met);
    assert_eq!(outcome("<= 2 s", "3"), Outcome::NotMeasurable);
    assert_eq!(outcome(">= 9", "1,000"), Outcome::Met);
}

#[test]
fn empty_output_is_not_measurable() {
    assert_eq!(outcome(">= 9", ""), Outcome::NotMeasurable);
}

#[test]
fn sentence_expectation_is_never_met() {
    let j = judge("exit code 2 while running; turn ends when complete", "ok");
    assert_eq!(j.outcome, Outcome::NotMet);
    assert_eq!(j.actual, "ok");
}
