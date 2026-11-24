//! A sliding-window rate limiter for one provider: at most `max_requests`
//! admissions in any window of length `window`.
//!
//! The limiter holds no clock and does no I/O. Every call takes the current
//! `Instant`, so tests drive time explicitly. A caller picks a [`Policy`]:
//! [`Policy::Reject`] refuses a call over the limit, and [`Policy::Wait`]
//! reserves the next free slot and returns how long to sleep before sending.
//! A reserved slot counts against the limit even if the caller never sends,
//! so a caller that abandons a wait must not reserve one.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use thiserror::Error;

/// What to do when the window is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Refuse with [`RateLimitError::Limited`], which says when a slot frees.
    Reject,
    /// Reserve the next free slot and return how long to wait before sending.
    Wait,
}

/// Why a call was not admitted, or why the limiter was built wrong.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RateLimitError {
    #[error("invalid rate limit: {0}")]
    InvalidConfig(&'static str),
    #[error("rate limit reached; a slot frees in {retry_after:?}")]
    Limited { retry_after: Duration },
    #[error("rate limit slot time is outside the range of the clock")]
    Unrepresentable,
}

/// Sliding-window limiter. Admission times are kept in ascending order.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    max: usize,
    window: Duration,
    /// Admission times, ascending. Reserved future slots are included.
    slots: VecDeque<Instant>,
}

impl RateLimiter {
    /// Builds a limiter that admits `max_requests` calls per `window`.
    ///
    /// # Errors
    ///
    /// [`RateLimitError::InvalidConfig`] when `max_requests` or `window` is zero.
    pub fn new(max_requests: u32, window: Duration) -> Result<Self, RateLimitError> {
        if max_requests == 0 {
            return Err(RateLimitError::InvalidConfig("max requests must be at least 1"));
        }
        if window.is_zero() {
            return Err(RateLimitError::InvalidConfig("window must be longer than zero"));
        }
        Ok(Self {
            max: usize::try_from(max_requests).unwrap_or(usize::MAX),
            window,
            slots: VecDeque::new(),
        })
    }

    /// Admits one call at `now`. Returns how long the caller must wait before
    /// sending: zero when a slot is free now.
    ///
    /// `now` must not move backwards between calls; an earlier `now` is treated
    /// as the same instant.
    ///
    /// # Errors
    ///
    /// [`RateLimitError::Limited`] when the window is full and `policy` is
    /// [`Policy::Reject`]. Nothing is recorded in that case. [`RateLimitError::Unrepresentable`]
    /// when the slot time cannot be represented by the clock.
    pub fn acquire(&mut self, now: Instant, policy: Policy) -> Result<Duration, RateLimitError> {
        self.expire(now);
        let slot = self.next_slot(now)?;
        let wait = slot.saturating_duration_since(now);
        if !wait.is_zero() && policy == Policy::Reject {
            return Err(RateLimitError::Limited { retry_after: wait });
        }
        let at = self.slots.partition_point(|s| *s <= slot);
        self.slots.insert(at, slot);
        Ok(wait)
    }

    /// Number of slots counted against the limit at `now`, including reserved ones.
    #[must_use]
    pub fn in_window(&self, now: Instant) -> usize {
        self.slots
            .iter()
            .filter(|s| now.saturating_duration_since(**s) < self.window)
            .count()
    }

    /// Drops slots that have left the window. Sorted order makes the oldest the front.
    fn expire(&mut self, now: Instant) {
        while let Some(oldest) = self.slots.front() {
            if now.saturating_duration_since(*oldest) < self.window {
                break;
            }
            self.slots.pop_front();
        }
    }

    /// The earliest time at or after `now` that a new slot may start. With fewer
    /// than `max` slots recorded, that is `now`. Otherwise the new slot may start
    /// once the `max`-th most recent slot leaves the window.
    fn next_slot(&self, now: Instant) -> Result<Instant, RateLimitError> {
        match self
            .slots
            .len()
            .checked_sub(self.max)
            .and_then(|i| self.slots.get(i))
        {
            None => Ok(now),
            Some(blocker) => blocker
                .checked_add(self.window)
                .map(|free| free.max(now))
                .ok_or(RateLimitError::Unrepresentable),
        }
    }
}

#[cfg(test)]
#[path = "ratelimit_test.rs"]
mod ratelimit_test;
