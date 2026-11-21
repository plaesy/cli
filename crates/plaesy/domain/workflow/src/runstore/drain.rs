//! Shutdown drain and retry queue for the run log (spec R3, VS-10).
//!
//! [`DrainingLog`] keeps records in order until they are written. After
//! [`DrainingLog::shutdown`] it accepts nothing new. [`RetryQueue`] retries failed
//! writes without reordering: a failing item blocks the items behind it.

use super::{RunLog, RunLogError};
use std::collections::VecDeque;

/// Refusal from a draining log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainError {
    /// Shutdown already started. No new record is accepted.
    Closed,
}

/// Run log with an ordered pending queue and a shutdown drain.
#[derive(Debug)]
pub struct DrainingLog {
    log: RunLog,
    pending: VecDeque<String>,
    closed: bool,
}

impl DrainingLog {
    /// Wraps a log. Nothing is written until [`DrainingLog::flush`].
    #[must_use]
    pub fn new(log: RunLog) -> Self {
        Self {
            log,
            pending: VecDeque::new(),
            closed: false,
        }
    }

    /// Queues one record behind the pending ones.
    ///
    /// # Errors
    ///
    /// Returns [`DrainError::Closed`] after shutdown started.
    pub fn enqueue(&mut self, payload: &str) -> Result<(), DrainError> {
        if self.closed {
            return Err(DrainError::Closed);
        }
        self.pending.push_back(payload.to_owned());
        Ok(())
    }

    /// Number of records still waiting to be written.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Writes pending records in order. Stops at the first write error and keeps the rest.
    ///
    /// # Errors
    ///
    /// Returns the [`RunLogError`] of the first failed write.
    pub fn flush(&mut self) -> Result<usize, RunLogError> {
        let mut written = 0;
        while let Some(payload) = self.pending.front().cloned() {
            self.log.append(&payload)?;
            self.pending.pop_front();
            written += 1;
        }
        Ok(written)
    }

    /// Drains pending records and closes the log. If a write fails, the log stays open.
    ///
    /// # Errors
    ///
    /// Returns the [`RunLogError`] of the first failed write.
    pub fn shutdown(&mut self) -> Result<usize, RunLogError> {
        let written = self.flush()?;
        self.closed = true;
        Ok(written)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    payload: String,
    attempts: u32,
}

/// Ordered retry queue. Items that exhaust their attempts move to dead letters.
#[derive(Debug)]
pub struct RetryQueue {
    items: VecDeque<Item>,
    max_attempts: u32,
    dead: Vec<String>,
}

impl RetryQueue {
    /// Creates a queue that gives each item at most `max_attempts` tries.
    #[must_use]
    pub fn new(max_attempts: u32) -> Self {
        Self {
            items: VecDeque::new(),
            max_attempts,
            dead: Vec::new(),
        }
    }

    /// Adds an item at the back.
    pub fn push(&mut self, payload: &str) {
        self.items.push_back(Item {
            payload: payload.to_owned(),
            attempts: 0,
        });
    }

    /// Items still waiting for a successful write.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when nothing is waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Payloads that exhausted their attempts, in the order they were dropped.
    #[must_use]
    pub fn dead_letters(&self) -> &[String] {
        &self.dead
    }

    /// Tries items in order. A failure stops the pass and counts one attempt against the front item.
    /// Returns how many items were written.
    pub fn run<E>(&mut self, mut write: impl FnMut(&str) -> Result<(), E>) -> usize {
        let mut delivered = 0;
        while let Some(front) = self.items.front_mut() {
            match write(&front.payload) {
                Ok(()) => {
                    self.items.pop_front();
                    delivered += 1;
                }
                Err(_) => {
                    front.attempts += 1;
                    if front.attempts >= self.max_attempts {
                        if let Some(item) = self.items.pop_front() {
                            self.dead.push(item.payload);
                        }
                        continue;
                    }
                    break;
                }
            }
        }
        delivered
    }
}

#[cfg(test)]
#[path = "drain_test.rs"]
mod drain_test;
