//! Approval and external-effect gate (spec R9).
//!
//! Internal actions always pass. An external effect passes only when the
//! external-effect flag is on and the owner has approved that exact action.

pub mod approvals;

/// Whether an action changes anything outside the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Changes only Plaesy-owned state.
    Internal,
    /// Calls or changes something outside Plaesy.
    External,
}

/// Why an external effect was blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    /// The external-effect flag is off for this deployment.
    ExternalEffectsDisabled,
    /// No approval was given for this action.
    ApprovalRequired,
}

impl Block {
    /// Stable reason string for run records and audit.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::ExternalEffectsDisabled => "external_effects_disabled",
            Self::ApprovalRequired => "approval_required",
        }
    }
}

/// Decides whether an action may run.
///
/// # Errors
///
/// Returns the [`Block`] when an external effect is not allowed.
pub fn may_run(
    effect: Effect,
    approved: bool,
    external_effects_enabled: bool,
) -> Result<(), Block> {
    match effect {
        Effect::Internal => Ok(()),
        Effect::External if !external_effects_enabled => Err(Block::ExternalEffectsDisabled),
        Effect::External if !approved => Err(Block::ApprovalRequired),
        Effect::External => Ok(()),
    }
}

#[cfg(test)]
#[path = "policy_test.rs"]
mod policy_test;
