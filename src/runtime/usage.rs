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
mod tests {
    use super::*;

    #[test]
    fn bounded_runtime_usage_counts_started_steps() {
        let cancel = CancelFlag::new();
        let (outcome, usage) =
            supervise_metered(Budget { max_steps: 3 }, &cancel, |_| StepResult::Continue);
        assert_eq!(
            outcome,
            Outcome::Failed {
                reason: "budget_exceeded".into()
            }
        );
        assert_eq!(usage.steps, 3);
    }
}
