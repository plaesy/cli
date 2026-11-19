//! JSON-lines export of the audit log with secret values redacted (spec R9, VS-15).

use super::AuditLog;
use crate::secrets::SecretStore;

/// Exports every audit event as one JSON object per line. Known secret values are redacted.
#[must_use]
pub fn export_jsonl(log: &AuditLog, secrets: &SecretStore) -> String {
    let mut out = String::new();
    for event in log.events() {
        let line = serde_json::json!({
            "principal": event.principal,
            "project": event.project,
            "action": event.action,
            "allowed": event.allowed,
            "reason": event.reason,
        })
        .to_string();
        out.push_str(&secrets.redact(&line));
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[path = "export_test.rs"]
mod export_test;
