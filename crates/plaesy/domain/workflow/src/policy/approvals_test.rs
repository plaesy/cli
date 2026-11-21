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
