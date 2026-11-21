use super::*;

#[test]
fn task_run_evidence_round_trip_verifies_hash() {
    let dir = tempfile::tempdir().unwrap();
    let store = EvidenceStore::open(dir.path().join("evidence"));
    let reference = store.put(b"report body").unwrap();
    assert_eq!(store.get(&reference).unwrap(), b"report body");
}

#[test]
fn task_run_evidence_tampered_bytes_are_not_served() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("evidence");
    let store = EvidenceStore::open(&root);
    let reference = store.put(b"report body").unwrap();
    fs::write(root.join(&reference.sha256), b"changed").unwrap();
    assert_eq!(store.get(&reference), Err(EvidenceError::HashMismatch));
}

#[test]
fn task_run_evidence_missing_and_invalid_references_fail() {
    let dir = tempfile::tempdir().unwrap();
    let store = EvidenceStore::open(dir.path().join("evidence"));
    let missing = EvidenceRef {
        sha256: "a".repeat(64),
    };
    assert_eq!(store.get(&missing), Err(EvidenceError::Missing));
    let invalid = EvidenceRef {
        sha256: "not-a-hash".into(),
    };
    assert_eq!(store.get(&invalid), Err(EvidenceError::InvalidReference));
}
