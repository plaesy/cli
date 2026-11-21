//! Execution adapter contract and a deterministic fake (spec R4).
//!
//! An adapter reports one terminal [`Outcome`] per run. A success must carry
//! non-empty evidence. [`check_conformance`] runs the rules every adapter must meet.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Terminal outcome reported by an adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The run finished. Evidence identifies the produced artifact.
    Succeeded { evidence: String },
    /// The run failed with a reason.
    Failed { reason: String },
    /// The run exceeded its time budget.
    TimedOut,
    /// The run was cancelled before it finished.
    Cancelled,
}

/// Cooperative cancellation flag shared between the controller and an adapter.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    /// Creates an unset flag.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Returns true once cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Contract every execution backend implements.
pub trait ExecutionAdapter {
    /// Stable backend name, reported in run records.
    fn name(&self) -> &str;

    /// Runs one task to a terminal outcome. Must observe `cancel` before reporting success.
    fn execute(&mut self, task_id: &str, cancel: &CancelFlag) -> Outcome;
}

/// Scripted behavior of [`FakeAdapter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Script {
    /// Succeed with the given evidence.
    Succeed(String),
    /// Fail with the given reason.
    Fail(String),
    /// Report a timeout.
    Timeout,
    /// Cancel the run and report it.
    Cancel,
}

/// Deterministic adapter for tests and simulations. Makes no external calls.
#[derive(Debug, Clone)]
pub struct FakeAdapter {
    script: Script,
}

impl FakeAdapter {
    /// Creates a fake that follows `script`.
    #[must_use]
    pub fn new(script: Script) -> Self {
        Self { script }
    }
}

impl ExecutionAdapter for FakeAdapter {
    fn name(&self) -> &str {
        "fake"
    }

    fn execute(&mut self, _task_id: &str, cancel: &CancelFlag) -> Outcome {
        if cancel.is_cancelled() {
            return Outcome::Cancelled;
        }
        match &self.script {
            Script::Succeed(evidence) => Outcome::Succeeded {
                evidence: evidence.clone(),
            },
            Script::Fail(reason) => Outcome::Failed {
                reason: reason.clone(),
            },
            Script::Timeout => Outcome::TimedOut,
            Script::Cancel => {
                cancel.cancel();
                Outcome::Cancelled
            }
        }
    }
}

/// Returns the conformance violations of one outcome. Empty when conforming.
#[must_use]
pub fn outcome_violations(outcome: &Outcome) -> Vec<String> {
    match outcome {
        Outcome::Succeeded { evidence } if evidence.trim().is_empty() => {
            vec!["succeeded without evidence".to_owned()]
        }
        Outcome::Failed { reason } if reason.trim().is_empty() => {
            vec!["failed without a reason".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// Runs the conformance rules against an adapter with a fresh cancel flag.
/// Returns all violations found. Empty when the adapter conforms.
pub fn check_conformance(adapter: &mut dyn ExecutionAdapter) -> Vec<String> {
    let outcome = adapter.execute("conformance-probe", &CancelFlag::new());
    outcome_violations(&outcome)
}

#[cfg(test)]
#[path = "adapter_test.rs"]
mod adapter_test;
