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
