use super::*;
use crate::runstore::MemoryLog;

#[test]
fn approval_secret_approval_survives_restart() {
    let log = MemoryLog::default();
    ApprovalStore::new(log.clone())
        .approve("send-email-42")
        .unwrap();
    let restarted = ApprovalStore::new(log);
    assert!(restarted.is_approved("send-email-42").unwrap());
    assert!(!restarted.is_approved("other-action").unwrap());
}

#[test]
fn approval_secret_revocation_is_latest_and_wins() {
    let store = ApprovalStore::new(MemoryLog::default());
    store.approve("act-1").unwrap();
    store.revoke("act-1").unwrap();
    assert!(!store.is_approved("act-1").unwrap());
}

#[test]
fn approval_secret_action_id_with_separator_is_rejected() {
    let store = ApprovalStore::new(MemoryLog::default());
    assert_eq!(store.approve("a|b"), Err(RunLogError::InvalidPayload));
}
