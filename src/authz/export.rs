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
mod tests {
    use super::*;
    use crate::authz::{check, Action};
    use crate::identity::{IdentityStore, PrincipalId, ProjectId};

    #[test]
    fn approval_secret_audit_export_redacts_secret_values() {
        let mut store = IdentityStore::new();
        let id = PrincipalId::new("agent-sk-synthetic-999");
        store
            .register(id.clone(), ProjectId::new("project-a"))
            .unwrap();
        let mut log = AuditLog::new();
        check(
            &store,
            &mut log,
            &id,
            &ProjectId::new("project-a"),
            &Action::Submit,
        )
        .unwrap();
        let mut secrets = SecretStore::new();
        secrets.insert("provider_key", "sk-synthetic-999");
        let exported = export_jsonl(&log, &secrets);
        assert!(!exported.contains("sk-synthetic-999"));
        assert!(exported.contains("[REDACTED:provider_key]"));
        assert_eq!(exported.lines().count(), 1);
    }
}
