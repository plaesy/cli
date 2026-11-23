//! Unit tests for `ratelimit.rs`. Time is injected as `Instant` offsets from a
//! fixed base, so no test reads the wall clock.

use super::*;

fn limiter(max: u32, window_secs: u64) -> RateLimiter {
    RateLimiter::new(max, Duration::from_secs(window_secs)).unwrap()
}

fn at(base: Instant, secs: u64) -> Instant {
    base + Duration::from_secs(secs)
}

#[test]
fn zero_limit_or_zero_window_is_refused() {
    assert!(matches!(
        RateLimiter::new(0, Duration::from_secs(1)),
        Err(RateLimitError::InvalidConfig(_))
    ));
    assert!(matches!(
        RateLimiter::new(1, Duration::ZERO),
        Err(RateLimitError::InvalidConfig(_))
    ));
}

#[test]
fn admits_up_to_the_limit_then_refuses_inside_the_window() {
    let base = Instant::now();
    let mut rl = limiter(2, 10);
    assert_eq!(rl.acquire(base, Policy::Reject), Ok(Duration::ZERO));
    assert_eq!(rl.acquire(at(base, 1), Policy::Reject), Ok(Duration::ZERO));
    // The first slot (t=0) leaves the window at t=10, so the refusal waits 8s from t=2.
    assert_eq!(
        rl.acquire(at(base, 2), Policy::Reject),
        Err(RateLimitError::Limited {
            retry_after: Duration::from_secs(8)
        })
    );
}

#[test]
fn a_refused_call_takes_no_slot() {
    let base = Instant::now();
    let mut rl = limiter(2, 10);
    rl.acquire(base, Policy::Reject).unwrap();
    rl.acquire(at(base, 1), Policy::Reject).unwrap();
    assert!(rl.acquire(at(base, 2), Policy::Reject).is_err());
    assert_eq!(rl.in_window(at(base, 2)), 2);
    // At t=10 the t=0 slot has expired, so one call fits. It fills the window with t=1 and t=10.
    assert_eq!(rl.acquire(at(base, 10), Policy::Reject), Ok(Duration::ZERO));
    // The t=1 slot leaves at t=11, so the next refusal is 1s.
    assert_eq!(
        rl.acquire(at(base, 10), Policy::Reject),
        Err(RateLimitError::Limited {
            retry_after: Duration::from_secs(1)
        })
    );
}

#[test]
fn capacity_refills_slot_by_slot_as_the_window_slides() {
    let base = Instant::now();
    let mut rl = limiter(2, 10);
    rl.acquire(base, Policy::Reject).unwrap();
    rl.acquire(at(base, 4), Policy::Reject).unwrap();
    assert!(rl.acquire(at(base, 5), Policy::Reject).is_err());

    // At t=10 only the t=0 slot has expired, so exactly one slot is free again.
    assert_eq!(rl.acquire(at(base, 10), Policy::Reject), Ok(Duration::ZERO));
    // The window is full again: t=4 and t=10. The t=4 slot leaves at t=14.
    assert_eq!(
        rl.acquire(at(base, 10), Policy::Reject),
        Err(RateLimitError::Limited {
            retry_after: Duration::from_secs(4)
        })
    );
}

#[test]
fn full_refill_after_a_whole_window_of_silence() {
    let base = Instant::now();
    let mut rl = limiter(3, 10);
    for _ in 0..3 {
        rl.acquire(base, Policy::Reject).unwrap();
    }
    assert!(rl.acquire(at(base, 9), Policy::Reject).is_err());
    assert_eq!(rl.in_window(at(base, 10)), 0);
    for _ in 0..3 {
        assert_eq!(rl.acquire(at(base, 10), Policy::Reject), Ok(Duration::ZERO));
    }
    assert!(rl.acquire(at(base, 10), Policy::Reject).is_err());
}

#[test]
fn wait_reserves_the_next_slot_and_reports_the_delay() {
    let base = Instant::now();
    let mut rl = limiter(1, 10);
    assert_eq!(rl.acquire(base, Policy::Wait), Ok(Duration::ZERO));
    // The single slot frees at t=10, so this call is told to wait 9s from t=1.
    assert_eq!(
        rl.acquire(at(base, 1), Policy::Wait),
        Ok(Duration::from_secs(9))
    );
    // That reservation holds the window until t=20, so a Reject now is refused for 18s.
    assert_eq!(
        rl.acquire(at(base, 2), Policy::Reject),
        Err(RateLimitError::Limited {
            retry_after: Duration::from_secs(18)
        })
    );
}

#[test]
fn waiting_calls_queue_behind_each_other_in_order() {
    let base = Instant::now();
    let mut rl = limiter(2, 10);
    assert_eq!(rl.acquire(base, Policy::Wait), Ok(Duration::ZERO));
    assert_eq!(rl.acquire(base, Policy::Wait), Ok(Duration::ZERO));
    // The third call takes the t=10 slot, the first one that frees.
    assert_eq!(rl.acquire(base, Policy::Wait), Ok(Duration::from_secs(10)));
    // A later waiter queues after that reservation, not before it.
    assert_eq!(
        rl.acquire(at(base, 5), Policy::Wait),
        Ok(Duration::from_secs(5))
    );
    // At t=10 both early slots have expired. Two reservations at t=10 fill the window,
    // so the next call must wait until t=20.
    assert_eq!(
        rl.acquire(at(base, 10), Policy::Reject),
        Err(RateLimitError::Limited {
            retry_after: Duration::from_secs(10)
        })
    );
}
