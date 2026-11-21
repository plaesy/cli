//! Unit tests for `tests` split out of `ratelimit.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_bucket_admits_its_burst_then_refuses() {
    let bucket = TokenBucket::new(2, 0.0001);
    assert!(bucket.allow());
    assert!(bucket.allow());
    assert!(!bucket.allow());
}

#[test]
fn a_window_log_admits_its_limit() {
    let log = SlidingWindowLog::new(2, Duration::from_secs(60));
    assert!(log.allow());
    assert!(log.allow());
    assert!(!log.allow());
    assert_eq!(log.remaining(), 0);
}

#[test]
fn a_wait_past_its_deadline_is_cancelled() {
    let bucket = TokenBucket::new(0, 0.0001);
    let deadline = Deadline::after(Duration::ZERO);
    assert_eq!(bucket.wait(deadline), Err(WaitCancelled));
}
