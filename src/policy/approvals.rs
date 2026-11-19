//! Persisted approvals for external actions (spec R9, approval persistence).
//!
//! Approvals and revocations are appended to a run log, so they survive a restart.
//! The latest record for an action id decides whether it is approved.

use crate::runstore::{RunLog, RunLogError};

/// Append-only approval record stored in a run log.
#[derive(Debug, Clone)]
pub struct ApprovalStore {
    log: RunLog,
}

impl ApprovalStore {
    /// Opens the approval record at `path`.
    #[must_use]
    pub fn open(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            log: RunLog::open(path),
        }
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
mod tests {
    use super::*;

    #[test]
    fn approval_secret_approval_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.log");
        ApprovalStore::open(&path).approve("send-email-42").unwrap();
        let restarted = ApprovalStore::open(&path);
        assert!(restarted.is_approved("send-email-42").unwrap());
        assert!(!restarted.is_approved("other-action").unwrap());
    }

    #[test]
    fn approval_secret_revocation_is_latest_and_wins() {
        let dir = tempfile::tempdir().unwrap();
        let store = ApprovalStore::open(dir.path().join("approvals.log"));
        store.approve("act-1").unwrap();
        store.revoke("act-1").unwrap();
        assert!(!store.is_approved("act-1").unwrap());
    }

    #[test]
    fn approval_secret_action_id_with_separator_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = ApprovalStore::open(dir.path().join("approvals.log"));
        assert_eq!(store.approve("a|b"), Err(RunLogError::InvalidPayload));
    }
}
