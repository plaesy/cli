//! Task states for the in-house agent protocol (see `docs/platform/agent-protocol-design.md`).
//!
//! The state machine is pure: it only says which transitions are allowed. Storage and transport are
//! separate, so this module needs no I/O and no dependency.

use std::fmt;

/// Lifecycle of one task sent from one agent to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Failed,
    Canceled,
}

impl TaskState {
    /// Stable wire name, used in JSON and in the relay store.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Working => "working",
            Self::InputRequired => "input-required",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    /// Parses a wire name. Returns `None` for anything unknown, so callers reject it.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "submitted" => Self::Submitted,
            "working" => Self::Working,
            "input-required" => Self::InputRequired,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "canceled" => Self::Canceled,
            _ => return None,
        })
    }

    /// A terminal task never changes again.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }

    /// Whether the task may move from `self` to `next`.
    pub fn can_move_to(self, next: Self) -> bool {
        use TaskState::{Canceled, Completed, Failed, InputRequired, Submitted, Working};
        matches!(
            (self, next),
            (Submitted, Working | Canceled | Failed)
                | (Working, InputRequired | Completed | Failed | Canceled)
                | (InputRequired, Working | Canceled | Failed)
        )
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Applies a transition, or explains why it is refused.
pub fn transition(from: TaskState, to: TaskState) -> Result<TaskState, String> {
    if from.can_move_to(to) {
        Ok(to)
    } else {
        Err(format!("task cannot move from {from} to {to}"))
    }
}


