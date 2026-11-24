//! Persisted approvals for external actions (spec R9, approval persistence).
//!
//! Approvals and revocations are appended to a run log, so they survive a restart.
//! The latest record for an action id decides whether it is approved.

use crate::runstore::{RunLogError, RunSink};

/// Append-only approval record stored in a run log.
#[derive(Debug, Clone)]
pub struct ApprovalStore<L: RunSink> {
    log: L,
}

impl<L: RunSink> ApprovalStore<L> {
    /// Creates the approval record on top of a run log.
    #[must_use]
    pub fn new(log: L) -> Self {
        Self { log }
    }

    /// Records approval for one action id.
    ///
    /// # Errors
    ///
    /// Returns [`RunLogError::InvalidPayload`] for ids that contain a separator or newline.
    pub fn approve(&self, action_id: &str) -> Result<(), RunLogError> {
        self.write("approve", action_id)
    }

    /// Records revocation of an earlier approval.
    ///
    /// # Errors
    ///
    /// Returns [`RunLogError::InvalidPayload`] for ids that contain a separator or newline.
    pub fn revoke(&self, action_id: &str) -> Result<(), RunLogError> {
        self.write("revoke", action_id)
    }

    /// Returns true when the latest record for the action is an approval.
    ///
    /// # Errors
    ///
    /// Returns the log error when the record cannot be read.
    pub fn is_approved(&self, action_id: &str) -> Result<bool, RunLogError> {
        let mut approved = false;
        for record in self.log.recover()?.records {
            let Some((verb, id)) = record.split_once('|') else {
                continue;
            };
            if id != action_id {
                continue;
            }
            match verb {
                "approve" => approved = true,
                "revoke" => approved = false,
                _ => {}
            }
        }
        Ok(approved)
    }

    fn write(&self, verb: &str, action_id: &str) -> Result<(), RunLogError> {
        if action_id.contains('|') {
            return Err(RunLogError::InvalidPayload);
        }
        self.log.append(&format!("{verb}|{action_id}"))
    }
}

#[cfg(test)]
#[path = "approvals_test.rs"]
mod approvals_test;
