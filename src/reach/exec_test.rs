//! Unit tests for `tests` split out of `exec.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_missing_program_is_a_failed_result_not_a_panic() {
    let result = run_backend(
        Platform::Web,
        "missing",
        &argv(&["plaesy-no-such-program-xyz"]),
        &[],
        Deadline::after(Duration::from_secs(5)),
    );
    assert!(!result.success);
    assert!(!result.error.is_empty());
}

#[test]
fn a_deadline_in_the_past_has_no_time_left() {
    let past = Deadline(Instant::now() - Duration::from_secs(1));
    assert!(past.remaining().is_none());
}
