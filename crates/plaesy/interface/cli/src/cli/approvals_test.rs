//! Tests for `approvals`: each decision lands in the audit log that `audit list` reads.

use super::*;

#[test]
fn approve_is_recorded_in_the_audit_log() {
    let root = tempfile::tempdir().unwrap();
    apply_at(
        root.path(),
        &Command::Approve {
            action_id: "tool:shell".into(),
        },
    )
    .unwrap();

    let listing = AuditTrail::new(RunLog::open(root.path().join(".plaesy/audit.log")))
        .list(&Default::default())
        .unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].action, "approval:approve");
    assert!(listing.entries[0].allowed);
}

#[test]
fn approval_check_records_the_result() {
    let root = tempfile::tempdir().unwrap();
    assert!(!is_approved_at(root.path(), "tool:shell").unwrap());
    audit_at(root.path(), "approval:check", false, "not approved").unwrap();

    let listing = AuditTrail::new(RunLog::open(root.path().join(".plaesy/audit.log")))
        .list(&Default::default())
        .unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert!(!listing.entries[0].allowed);
    assert_eq!(listing.entries[0].reason, "not approved");
}
