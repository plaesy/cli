//! Retry policy and per-step time budget for one delegate call.
//!
//! Every function here is pure: it takes the policy, the budget, and the
//! observed state, and returns a decision. Nothing sleeps and nothing reads a
//! clock, so the caller owns I/O and time. Backoff doubles per retry and is
//! capped at `max_delay`. A server `Retry-After` is a lower bound on the wait.

use std::time::Duration;

use thiserror::Error;

use super::Class;

/// Attempts allowed for one provider, counting the first call.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;
/// Wait before the first retry.
pub const DEFAULT_BASE_DELAY: Duration = Duration::from_secs(1);
/// Upper bound for any single wait, including a server `Retry-After`.
pub const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(30);

/// Doublings past this point are clamped so the shift never overflows.
const MAX_DOUBLINGS: u32 = 32;

/// A policy or budget that cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PolicyError {
    #[error("max_attempts must be at least 1")]
    NoAttempts,
    #[error("base_delay must not exceed max_delay")]
    DelayOrder,
    #[error("per-call and total time budgets must be non-zero")]
    ZeroBudget,
    #[error("per-call budget must not exceed the total budget")]
    CallExceedsTotal,
}

/// How many times one provider may be called and how long to wait between calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total calls allowed, including the first one. Zero is rejected by [`RetryPolicy::new`].
    pub max_attempts: u32,
    /// Wait before the first retry. Each later retry doubles it.
    pub base_delay: Duration,
    /// Cap for every wait.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            base_delay: DEFAULT_BASE_DELAY,
            max_delay: DEFAULT_MAX_DELAY,
        }
    }
}

impl RetryPolicy {
    /// Checks the fields and returns the policy.
    ///
    /// # Errors
    ///
    /// [`PolicyError::NoAttempts`] when `max_attempts` is zero, and
    /// [`PolicyError::DelayOrder`] when `base_delay` exceeds `max_delay`.
    pub fn new(
        max_attempts: u32,
        base_delay: Duration,
        max_delay: Duration,
    ) -> Result<Self, PolicyError> {
        if max_attempts == 0 {
            return Err(PolicyError::NoAttempts);
        }
        if base_delay > max_delay {
            return Err(PolicyError::DelayOrder);
        }
        Ok(Self {
            max_attempts,
            base_delay,
            max_delay,
        })
    }
}

/// Time allowed for one step: each call's limit and the total across calls and waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepBudget {
    /// Limit for a single provider call. Never zero, because a zero timeout means "no limit".
    pub per_call: Duration,
    /// Limit for the whole step, including waits. Never zero.
    pub total: Duration,
}

impl StepBudget {
    /// Checks the fields and returns the budget.
    ///
    /// # Errors
    ///
    /// [`PolicyError::ZeroBudget`] when either field is zero, and
    /// [`PolicyError::CallExceedsTotal`] when `per_call` exceeds `total`.
    pub fn new(per_call: Duration, total: Duration) -> Result<Self, PolicyError> {
        if per_call.is_zero() || total.is_zero() {
            return Err(PolicyError::ZeroBudget);
        }
        if per_call > total {
            return Err(PolicyError::CallExceedsTotal);
        }
        Ok(Self { per_call, total })
    }
}

/// Why a step stops instead of retrying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The failure class does not repeat on the same provider.
    Terminal,
    /// `max_attempts` calls have been made.
    AttemptsExhausted,
    /// The server asked for a longer wait than `max_delay` allows.
    RetryAfterTooLong,
    /// Waiting, then calling again, would pass the total budget.
    BudgetExhausted,
}

/// The decision after a failed call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Sleep for `wait`, then call again with `timeout` as the per-call limit.
    Retry { wait: Duration, timeout: Duration },
    /// Stop retrying this provider.
    Stop(StopReason),
}

/// Wait before retry number `retry` (1 is the first retry). Zero gives zero.
///
/// The wait is `base_delay * 2^(retry - 1)`, capped at `max_delay`.
#[must_use]
pub fn backoff_delay(policy: &RetryPolicy, retry: u32) -> Duration {
    if retry == 0 {
        return Duration::ZERO;
    }
    let doublings = (retry - 1).min(MAX_DOUBLINGS);
    let scaled = policy
        .base_delay
        .as_millis()
        .saturating_mul(1u128 << doublings);
    let capped = scaled.min(policy.max_delay.as_millis());
    Duration::from_millis(u64::try_from(capped).unwrap_or(u64::MAX))
}

/// Per-call limit at `elapsed` into the step: the smaller of `per_call` and what
/// remains of `total`. `None` once the total is used up.
#[must_use]
pub fn call_window(budget: &StepBudget, elapsed: Duration) -> Option<Duration> {
    let remaining = budget.total.checked_sub(elapsed)?;
    if remaining.is_zero() {
        return None;
    }
    Some(remaining.min(budget.per_call))
}

/// Decides what to do after call number `attempt` (1-based) failed with `class`.
///
/// Terminal failures stop at once. Otherwise the step waits `max(backoff,
/// retry_after)` for a rate limit and `backoff` for anything else, provided
/// attempts remain and the wait plus one more call fits in the total budget.
/// `elapsed` is the time the step has used so far, excluding the wait.
#[must_use]
pub fn next_step(
    policy: &RetryPolicy,
    budget: &StepBudget,
    class: Class,
    attempt: u32,
    retry_after: Option<Duration>,
    elapsed: Duration,
) -> Next {
    if class == Class::Terminal {
        return Next::Stop(StopReason::Terminal);
    }
    if attempt >= policy.max_attempts {
        return Next::Stop(StopReason::AttemptsExhausted);
    }
    let server_wait = match class {
        Class::RateLimited => retry_after.unwrap_or(Duration::ZERO),
        _ => Duration::ZERO,
    };
    if server_wait > policy.max_delay {
        return Next::Stop(StopReason::RetryAfterTooLong);
    }
    let wait = backoff_delay(policy, attempt).max(server_wait);
    let Some(resume_at) = elapsed.checked_add(wait) else {
        return Next::Stop(StopReason::BudgetExhausted);
    };
    let Some(timeout) = call_window(budget, resume_at) else {
        return Next::Stop(StopReason::BudgetExhausted);
    };
    Next::Retry { wait, timeout }
}

#[cfg(test)]
#[path = "retry_test.rs"]
mod retry_test;
