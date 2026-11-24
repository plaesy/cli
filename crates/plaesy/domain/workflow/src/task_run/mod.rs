//! Run lifecycle state machine with restart recovery (spec VS-01, VS-02, VS-09).
//!
//! Every transition is written to the run log before it is applied, so a restart
//! can rebuild the same state from the log alone.

use crate::runstore::{RunLogError, RunSink};

pub mod status;

/// Lifecycle state of one attempt of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState {
    Queued,
    Running,
    Succeeded { evidence: String },
    Failed { reason: String },
    TimedOut,
    Cancelled,
    Interrupted,
}

impl RunState {
    /// True when no further transition is allowed.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded { .. } | Self::Failed { .. } | Self::TimedOut | Self::Cancelled
        )
    }
}

/// An event that drives a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Start,
    Complete {
        evidence: String,
    },
    Fail {
        reason: String,
    },
    Timeout,
    Cancel,
    /// The controller restarted while the run was running.
    Restart,
    /// A new attempt is queued after an interruption.
    Requeue,
}

impl Event {
    fn encode(&self) -> String {
        match self {
            Self::Start => "start".to_owned(),
            Self::Complete { evidence } => format!("complete:{evidence}"),
            Self::Fail { reason } => format!("fail:{reason}"),
            Self::Timeout => "timeout".to_owned(),
            Self::Cancel => "cancel".to_owned(),
            Self::Restart => "restart".to_owned(),
            Self::Requeue => "requeue".to_owned(),
        }
    }

    fn decode(text: &str) -> Option<Self> {
        Some(match text {
            "start" => Self::Start,
            "timeout" => Self::Timeout,
            "cancel" => Self::Cancel,
            "restart" => Self::Restart,
            "requeue" => Self::Requeue,
            other => {
                if let Some(evidence) = other.strip_prefix("complete:") {
                    Self::Complete {
                        evidence: evidence.to_owned(),
                    }
                } else {
                    Self::Fail {
                        reason: other.strip_prefix("fail:")?.to_owned(),
                    }
                }
            }
        })
    }
}

/// Why a transition was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// The event is not allowed in the current state.
    Invalid { state: String, event: String },
    /// A success must carry evidence.
    EmptyEvidence,
    /// A failure must carry a reason.
    EmptyReason,
    /// The run log could not be written or read.
    Log(RunLogError),
}

impl From<RunLogError> for TransitionError {
    fn from(error: RunLogError) -> Self {
        Self::Log(error)
    }
}

/// Pure transition function. Returns the next state or the refusal.
///
/// # Errors
///
/// Returns a [`TransitionError`] when the event is not allowed or its payload is empty.
pub fn transition(state: &RunState, event: &Event) -> Result<RunState, TransitionError> {
    let invalid = || TransitionError::Invalid {
        state: format!("{state:?}"),
        event: event.encode(),
    };
    match (state, event) {
        (RunState::Queued, Event::Start) => Ok(RunState::Running),
        (RunState::Running, Event::Complete { evidence }) => {
            if evidence.trim().is_empty() {
                Err(TransitionError::EmptyEvidence)
            } else {
                Ok(RunState::Succeeded {
                    evidence: evidence.clone(),
                })
            }
        }
        (RunState::Running, Event::Fail { reason }) => {
            if reason.trim().is_empty() {
                Err(TransitionError::EmptyReason)
            } else {
                Ok(RunState::Failed {
                    reason: reason.clone(),
                })
            }
        }
        (RunState::Running, Event::Timeout) => Ok(RunState::TimedOut),
        (RunState::Queued | RunState::Running, Event::Cancel) => Ok(RunState::Cancelled),
        (RunState::Running, Event::Restart) => Ok(RunState::Interrupted),
        (RunState::Interrupted, Event::Requeue) => Ok(RunState::Queued),
        (RunState::Interrupted, Event::Fail { reason }) => {
            if reason.trim().is_empty() {
                Err(TransitionError::EmptyReason)
            } else {
                Ok(RunState::Failed {
                    reason: reason.clone(),
                })
            }
        }
        _ => Err(invalid()),
    }
}

/// One run: a task, an attempt counter, the current state, and its retry budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub task_id: String,
    pub run_id: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub state: RunState,
}

impl Run {
    /// Creates a queued run at attempt 1.
    #[must_use]
    pub fn new(task_id: &str, run_id: &str, max_attempts: u32) -> Self {
        Self {
            task_id: task_id.to_owned(),
            run_id: run_id.to_owned(),
            attempt: 1,
            max_attempts,
            state: RunState::Queued,
        }
    }

    /// Applies an event in memory only.
    ///
    /// # Errors
    ///
    /// Returns the [`TransitionError`] from [`transition`].
    pub fn apply(&mut self, event: &Event) -> Result<(), TransitionError> {
        let next = transition(&self.state, event)?;
        if *event == Event::Requeue {
            self.attempt += 1;
        }
        self.state = next;
        Ok(())
    }

    /// Writes the event to the log first, then applies it. Nothing changes if the write fails.
    ///
    /// # Errors
    ///
    /// Returns [`TransitionError::Log`] when the write fails, or the transition error.
    pub fn apply_logged(
        &mut self,
        log: &impl RunSink,
        event: &Event,
    ) -> Result<(), TransitionError> {
        transition(&self.state, event)?;
        log.append(&self.record(event))?;
        self.apply(event)
    }

    fn record(&self, event: &Event) -> String {
        format!(
            "{}|{}|{}|{}",
            self.task_id,
            self.run_id,
            self.attempt,
            event.encode()
        )
    }
}

/// Rebuilds a run from the log after a restart.
///
/// A run left in `Running` is marked `Interrupted`. It is requeued as a new attempt
/// while attempts remain, and failed with reason `retry_exhausted` otherwise. Each
/// recovery step is logged before it is applied.
///
/// # Errors
///
/// Returns a [`TransitionError`] when the log cannot be read or contains an impossible sequence.
pub fn recover(
    log: &impl RunSink,
    task_id: &str,
    run_id: &str,
    max_attempts: u32,
) -> Result<Run, TransitionError> {
    let mut run = Run::new(task_id, run_id, max_attempts);
    for record in log.recover()?.records {
        let mut parts = record.splitn(4, '|');
        let (Some(task), Some(run_name), Some(_attempt), Some(encoded)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if task != task_id || run_name != run_id {
            continue;
        }
        let event = Event::decode(encoded).ok_or_else(|| TransitionError::Invalid {
            state: format!("{:?}", run.state),
            event: encoded.to_owned(),
        })?;
        run.apply(&event)?;
    }
    if run.state == RunState::Running {
        run.apply_logged(log, &Event::Restart)?;
    }
    if run.state == RunState::Interrupted {
        if run.attempt < run.max_attempts {
            run.apply_logged(log, &Event::Requeue)?;
        } else {
            run.apply_logged(
                log,
                &Event::Fail {
                    reason: "retry_exhausted".to_owned(),
                },
            )?;
        }
    }
    Ok(run)
}

#[cfg(test)]
#[path = "task_run_test.rs"]
mod task_run_test;
