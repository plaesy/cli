//! Usage reporting for supervised runs (spec R9 usage and cost reporting).
//!
//! Wraps [`super::supervise`] and reports how many steps started and how long the
//! run took. Cost is reported as steps and elapsed time only. No money figure is
//! computed here, because pricing is an owner decision.

use super::{supervise, Budget, StepResult};
use crate::adapter::{CancelFlag, Outcome};
use std::time::{Duration, Instant};

/// Usage of one supervised run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    /// Steps started by the supervisor.
    pub steps: u32,
    /// Wall-clock time spent in the supervisor.
    pub elapsed: Duration,
}

/// Runs the supervisor and reports usage alongside the outcome.
pub fn supervise_metered(
    budget: Budget,
    cancel: &CancelFlag,
    mut step: impl FnMut(u32) -> StepResult,
) -> (Outcome, Usage) {
    let started = Instant::now();
    let (outcome, steps) = supervise(budget, cancel, &mut step);
    (
        outcome,
        Usage {
            steps,
            elapsed: started.elapsed(),
        },
    )
}

#[cfg(test)]
#[path = "usage_test.rs"]
mod usage_test;
