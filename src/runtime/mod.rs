//! Bounded supervised execution (spec R9, VS-06, VS-07, VS-08).
//!
//! The supervisor runs steps one at a time. It stops at the step budget, at
//! cancellation, or at the first terminal step result. External calls go through
//! the policy gate and are blocked while the external-effect flag is off.

use crate::adapter::{CancelFlag, Outcome};
use crate::policy::{self, Block, Effect};

/// Step budget for one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Maximum number of steps the supervisor may start.
    pub max_steps: u32,
}

/// Result of one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    /// The step is done and the run should continue.
    Continue,
    /// The run is finished with evidence.
    Done { evidence: String },
    /// The step failed and the run must stop.
    Failed { reason: String },
}

/// Runs steps until a terminal outcome, the budget, or cancellation.
///
/// Returns the outcome and the number of steps started. A budget exhaustion is
/// reported as `Failed` with reason `budget_exceeded`.
pub fn supervise(
    budget: Budget,
    cancel: &CancelFlag,
    mut step: impl FnMut(u32) -> StepResult,
) -> (Outcome, u32) {
    for index in 0..budget.max_steps {
        if cancel.is_cancelled() {
            return (Outcome::Cancelled, index);
        }
        let started = index + 1;
        match step(index) {
            StepResult::Continue => {}
            StepResult::Done { evidence } if evidence.trim().is_empty() => {
                return (
                    Outcome::Failed {
                        reason: "empty_evidence".to_owned(),
                    },
                    started,
                );
            }
            StepResult::Done { evidence } => {
                return (Outcome::Succeeded { evidence }, started);
            }
            StepResult::Failed { reason } => {
                return (Outcome::Failed { reason }, started);
            }
        }
    }
    if cancel.is_cancelled() {
        return (Outcome::Cancelled, budget.max_steps);
    }
    (
        Outcome::Failed {
            reason: "budget_exceeded".to_owned(),
        },
        budget.max_steps,
    )
}

/// Performs an external call only if the policy gate allows it.
///
/// # Errors
///
/// Returns the [`Block`] when the call is not allowed. The call is not made.
pub fn external_call<T>(
    external_effects_enabled: bool,
    approved: bool,
    call: impl FnOnce() -> T,
) -> Result<T, Block> {
    policy::may_run(Effect::External, approved, external_effects_enabled)?;
    Ok(call())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_runtime_budget_exhaustion_stops_before_next_step() {
        let cancel = CancelFlag::new();
        let mut calls = 0;
        let (outcome, steps) = supervise(Budget { max_steps: 2 }, &cancel, |_| {
            calls += 1;
            StepResult::Continue
        });
        assert_eq!(
            outcome,
            Outcome::Failed {
                reason: "budget_exceeded".into()
            }
        );
        assert_eq!(steps, 2);
        assert_eq!(calls, 2);
    }

    #[test]
    fn bounded_runtime_cancel_stops_before_next_step() {
        let cancel = CancelFlag::new();
        let mut calls = 0;
        let (outcome, steps) = supervise(Budget { max_steps: 10 }, &cancel, |index| {
            calls += 1;
            if index == 2 {
                cancel.cancel();
            }
            StepResult::Continue
        });
        assert_eq!(outcome, Outcome::Cancelled);
        assert_eq!(steps, 3);
        assert_eq!(calls, 3);
    }

    #[test]
    fn bounded_runtime_done_step_reports_success_with_evidence() {
        let cancel = CancelFlag::new();
        let (outcome, steps) = supervise(Budget { max_steps: 5 }, &cancel, |index| {
            if index == 1 {
                StepResult::Done {
                    evidence: "artifact:1".into(),
                }
            } else {
                StepResult::Continue
            }
        });
        assert_eq!(
            outcome,
            Outcome::Succeeded {
                evidence: "artifact:1".into()
            }
        );
        assert_eq!(steps, 2);
    }

    #[test]
    fn bounded_runtime_external_call_blocked_when_flag_off() {
        let mut made = false;
        let result = external_call(false, true, || made = true);
        assert_eq!(result, Err(Block::ExternalEffectsDisabled));
        assert!(!made);
    }
}
