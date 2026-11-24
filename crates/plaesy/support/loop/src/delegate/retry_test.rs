//! Unit tests for `retry.rs`. Included via `#[path]`, so they see its items through `super`.

use super::*;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn policy() -> RetryPolicy {
    RetryPolicy::new(3, secs(1), secs(30)).unwrap()
}

fn budget(per_call: u64, total: u64) -> StepBudget {
    StepBudget::new(secs(per_call), secs(total)).unwrap()
}

#[test]
fn policy_and_budget_constructors_reject_unusable_values() {
    assert_eq!(
        RetryPolicy::new(0, secs(1), secs(2)),
        Err(PolicyError::NoAttempts)
    );
    assert_eq!(
        RetryPolicy::new(2, secs(3), secs(2)),
        Err(PolicyError::DelayOrder)
    );
    assert!(RetryPolicy::new(1, Duration::ZERO, Duration::ZERO).is_ok());
    assert_eq!(
        StepBudget::new(Duration::ZERO, secs(5)),
        Err(PolicyError::ZeroBudget)
    );
    assert_eq!(
        StepBudget::new(secs(5), Duration::ZERO),
        Err(PolicyError::ZeroBudget)
    );
    assert_eq!(
        StepBudget::new(secs(6), secs(5)),
        Err(PolicyError::CallExceedsTotal)
    );
    assert!(StepBudget::new(secs(5), secs(5)).is_ok());
}

#[test]
fn default_policy_is_the_published_default() {
    let p = RetryPolicy::default();
    assert_eq!(p.max_attempts, DEFAULT_MAX_ATTEMPTS);
    assert_eq!(p.base_delay, DEFAULT_BASE_DELAY);
    assert_eq!(p.max_delay, DEFAULT_MAX_DELAY);
}

#[test]
fn backoff_doubles_per_retry_and_is_capped() {
    let p = policy();
    let waits: Vec<Duration> = (1..=7).map(|r| backoff_delay(&p, r)).collect();
    assert_eq!(
        waits,
        [
            secs(1),
            secs(2),
            secs(4),
            secs(8),
            secs(16),
            secs(30),
            secs(30)
        ]
    );
}

#[test]
fn backoff_zero_retry_is_zero_and_huge_retry_does_not_overflow() {
    let p = policy();
    assert_eq!(backoff_delay(&p, 0), Duration::ZERO);
    assert_eq!(backoff_delay(&p, u32::MAX), secs(30));
}

#[test]
fn backoff_with_zero_base_stays_zero() {
    let p = RetryPolicy::new(5, Duration::ZERO, secs(30)).unwrap();
    assert_eq!(backoff_delay(&p, 4), Duration::ZERO);
}

#[test]
fn call_window_is_the_smaller_of_per_call_and_remaining_total() {
    let b = budget(10, 25);
    assert_eq!(call_window(&b, Duration::ZERO), Some(secs(10)));
    assert_eq!(call_window(&b, secs(20)), Some(secs(5)));
    assert_eq!(call_window(&b, secs(25)), None);
    assert_eq!(call_window(&b, secs(30)), None);
}

#[test]
fn terminal_failure_never_retries() {
    let next = next_step(
        &policy(),
        &budget(10, 60),
        Class::Terminal,
        1,
        None,
        Duration::ZERO,
    );
    assert_eq!(next, Next::Stop(StopReason::Terminal));
}

#[test]
fn transient_failure_backs_off_until_attempts_run_out() {
    let p = policy();
    let b = budget(10, 60);
    assert_eq!(
        next_step(&p, &b, Class::Transient, 1, None, Duration::ZERO),
        Next::Retry {
            wait: secs(1),
            timeout: secs(10)
        }
    );
    assert_eq!(
        next_step(&p, &b, Class::Transient, 2, None, secs(1)),
        Next::Retry {
            wait: secs(2),
            timeout: secs(10)
        }
    );
    assert_eq!(
        next_step(&p, &b, Class::Transient, 3, None, secs(3)),
        Next::Stop(StopReason::AttemptsExhausted)
    );
}

#[test]
fn rate_limit_waits_at_least_retry_after() {
    let p = policy();
    let b = budget(10, 60);
    // Retry-After longer than the backoff wins.
    assert_eq!(
        next_step(&p, &b, Class::RateLimited, 1, Some(secs(5)), Duration::ZERO),
        Next::Retry {
            wait: secs(5),
            timeout: secs(10)
        }
    );
    // With four attempts allowed, attempt 3 backs off 4s, which beats a 1s Retry-After.
    let p4 = RetryPolicy::new(4, secs(1), secs(30)).unwrap();
    assert_eq!(
        next_step(
            &p4,
            &b,
            Class::RateLimited,
            3,
            Some(secs(1)),
            Duration::ZERO
        ),
        Next::Retry {
            wait: secs(4),
            timeout: secs(10)
        }
    );
}

#[test]
fn retry_after_above_max_delay_stops_the_step() {
    let next = next_step(
        &policy(),
        &budget(10, 600),
        Class::RateLimited,
        1,
        Some(secs(31)),
        Duration::ZERO,
    );
    assert_eq!(next, Next::Stop(StopReason::RetryAfterTooLong));
}

#[test]
fn wait_that_leaves_no_budget_for_another_call_stops_the_step() {
    // Elapsed 4s + 1s wait = 5s, which uses the whole 5s total.
    let next = next_step(&policy(), &budget(2, 5), Class::Transient, 1, None, secs(4));
    assert_eq!(next, Next::Stop(StopReason::BudgetExhausted));
}

#[test]
fn retry_timeout_is_capped_by_what_remains_of_the_total() {
    let p = policy();
    // Total 12s, per call 10s, wait 1s: 11s remain, so the call gets its 10s limit.
    let b = budget(10, 12);
    assert_eq!(
        next_step(&p, &b, Class::Transient, 1, None, Duration::ZERO),
        Next::Retry {
            wait: secs(1),
            timeout: secs(10)
        }
    );
    // Total 12s, per call 10s, elapsed 4s and wait 1s: 7s remain, so the limit is 7s.
    assert_eq!(
        next_step(&p, &b, Class::Transient, 1, None, secs(4)),
        Next::Retry {
            wait: secs(1),
            timeout: secs(7)
        }
    );
}

#[test]
fn elapsed_overflow_stops_instead_of_panicking() {
    let next = next_step(
        &policy(),
        &budget(10, 60),
        Class::Transient,
        1,
        None,
        Duration::MAX,
    );
    assert_eq!(next, Next::Stop(StopReason::BudgetExhausted));
}
