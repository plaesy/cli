//! Unit tests for `tests` split out of `router.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn retryable_matching_ignores_case() {
    let retry = RetryConfig::default();
    assert!(is_retryable("Connection Refused by host", &retry));
    assert!(!is_retryable("invalid argument", &retry));
}

#[test]
fn backoff_stays_under_the_maximum_with_jitter() {
    let retry = RetryConfig::default();
    let delay = backoff(20, &retry);
    assert!(delay <= retry.max_delay.mul_f64(1.25));
}

#[test]
fn mutations_are_not_cacheable() {
    let mut opts = Options::new();
    opts.insert("action".into(), "post".into());
    assert!(!is_cacheable(&opts));
    assert!(is_cacheable(&Options::new()));
}
