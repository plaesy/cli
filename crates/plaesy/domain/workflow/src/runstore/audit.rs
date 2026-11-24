//! Audit trail stored in its own run log (spec VS-15, VS-16).
//!
//! Each entry is one run-log payload with six `|`-separated fields:
//! `<at>|<principal>|<project>|<action>|<allow|deny>|<reason>`.
//! `at` is unix seconds supplied by the caller, so this module never reads the clock.
//! `reason` is empty for an allowed decision. No field may contain `|` or a newline.
//!
//! This layout is a default of this module. It is not the in-memory `AuditEvent` in
//! `plaesy-domain-identity`, and the existing run-log formats are unchanged.

use super::{RunLog, RunLogError};
use std::fmt;
use std::path::PathBuf;

/// One audit entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    /// Unix seconds when the decision was made.
    pub at: u64,
    /// Principal that attempted the action.
    pub principal: String,
    /// Project the action targeted.
    pub project: String,
    /// Action label, as produced by the caller.
    pub action: String,
    /// True when the action was allowed.
    pub allowed: bool,
    /// Denial reason. Empty when the action was allowed.
    pub reason: String,
}

/// Errors from the audit trail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// The underlying run log failed: I/O, invalid payload, or checksum mismatch.
    Log(RunLogError),
    /// A checksum-valid record does not follow the audit layout (1-based record number).
    Malformed { record: usize },
}

impl From<RunLogError> for AuditError {
    fn from(error: RunLogError) -> Self {
        Self::Log(error)
    }
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Log(error) => write!(f, "audit log: {error}"),
            Self::Malformed { record } => write!(f, "audit record {record} is malformed"),
        }
    }
}

impl std::error::Error for AuditError {}

/// Filter and limit for [`AuditTrail::list`]. Unset fields do not filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditQuery {
    /// Keep only entries whose action label equals this value.
    pub action: Option<String>,
    /// Keep only entries at or after this unix second (inclusive).
    pub since: Option<u64>,
    /// Keep only entries at or before this unix second (inclusive).
    pub until: Option<u64>,
    /// Return at most this many entries. Unset returns all matches.
    pub limit: Option<usize>,
}

impl AuditQuery {
    fn matches(&self, record: &AuditRecord) -> bool {
        if self
            .action
            .as_ref()
            .is_some_and(|action| *action != record.action)
        {
            return false;
        }
        if self.since.is_some_and(|since| record.at < since) {
            return false;
        }
        if self.until.is_some_and(|until| record.at > until) {
            return false;
        }
        true
    }
}

/// Result of [`AuditTrail::list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditListing {
    /// Matching entries, newest first (reverse of append order), at most the query limit.
    pub entries: Vec<AuditRecord>,
    /// True when the log ends in an incomplete line that was ignored.
    pub torn_tail: bool,
}

/// Append-only audit trail stored in one run log file.
#[derive(Debug, Clone)]
pub struct AuditTrail {
    log: RunLog,
}

impl AuditTrail {
    /// Opens the trail at `path`. The file is created on first append.
    #[must_use]
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self {
            log: RunLog::open(path),
        }
    }

    /// Appends one entry and syncs it to disk before returning.
    ///
    /// # Errors
    ///
    /// Returns [`AuditError::Log`] with [`RunLogError::InvalidPayload`] when a field contains
    /// `|` or a newline, or with an I/O error when the write fails.
    pub fn append(&self, record: &AuditRecord) -> Result<(), AuditError> {
        let fields = [
            &record.principal,
            &record.project,
            &record.action,
            &record.reason,
        ];
        if fields
            .iter()
            .any(|field| field.chars().any(|c| c == '|' || c == '\n'))
        {
            return Err(AuditError::Log(RunLogError::InvalidPayload));
        }
        let verdict = if record.allowed { "allow" } else { "deny" };
        let payload = format!(
            "{}|{}|{}|{}|{}|{}",
            record.at, record.principal, record.project, record.action, verdict, record.reason
        );
        self.log.append(&payload).map_err(AuditError::from)
    }

    /// Reads every entry, checks each one against the audit layout, and returns the matches
    /// newest first.
    ///
    /// # Errors
    ///
    /// Returns [`AuditError::Log`] when the run log is corrupt, or [`AuditError::Malformed`]
    /// when a checksum-valid record does not parse. Neither is skipped silently.
    pub fn list(&self, query: &AuditQuery) -> Result<AuditListing, AuditError> {
        let recovered = self.log.recover()?;
        let mut parsed = Vec::with_capacity(recovered.records.len());
        for (index, payload) in recovered.records.iter().enumerate() {
            let record = parse(payload).ok_or(AuditError::Malformed { record: index + 1 })?;
            parsed.push(record);
        }
        let entries = parsed
            .into_iter()
            .rev()
            .filter(|record| query.matches(record))
            .take(query.limit.unwrap_or(usize::MAX))
            .collect();
        Ok(AuditListing {
            entries,
            torn_tail: recovered.torn_tail,
        })
    }
}

fn parse(payload: &str) -> Option<AuditRecord> {
    let mut parts = payload.splitn(6, '|');
    let at = parts.next()?.parse::<u64>().ok()?;
    let principal = parts.next()?.to_owned();
    let project = parts.next()?.to_owned();
    let action = parts.next()?.to_owned();
    let allowed = match parts.next()? {
        "allow" => true,
        "deny" => false,
        _ => return None,
    };
    let reason = parts.next()?.to_owned();
    Some(AuditRecord {
        at,
        principal,
        project,
        action,
        allowed,
        reason,
    })
}

#[cfg(test)]
#[path = "audit_test.rs"]
mod audit_test;
