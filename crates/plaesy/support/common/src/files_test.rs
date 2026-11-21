//! Unit tests for `tests` split out of `files.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn claim_is_visible_and_conflicting_claim_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let first = claim(root.path(), "src/lib.rs", "agent-a", None, 60).unwrap();
    assert_eq!(
        list(root.path(), Some("src/lib.rs")).unwrap(),
        vec![first.clone()]
    );
    assert!(matches!(
        claim(root.path(), "src/lib.rs", "agent-b", None, 60),
        Err(Error::Conflict { .. })
    ));
}

#[test]
fn concurrent_claims_have_one_winner() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let root = tempfile::tempdir().unwrap();
    let root = Arc::new(root.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(3));
    let mut threads = Vec::new();
    for agent in ["agent-a", "agent-b"] {
        let root = Arc::clone(&root);
        let barrier = Arc::clone(&barrier);
        threads.push(thread::spawn(move || {
            barrier.wait();
            claim(&root, "src/lib.rs", agent, None, 60)
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(Error::Conflict { .. })))
            .count(),
        1
    );
    assert_eq!(list(&root, None).unwrap().len(), 1);
}

#[test]
fn only_owner_can_release_claim() {
    let root = tempfile::tempdir().unwrap();
    claim(root.path(), "src/lib.rs", "agent-a", None, 60).unwrap();
    assert!(matches!(
        release(root.path(), "src/lib.rs", "agent-b"),
        Err(Error::NotOwner { .. })
    ));
    release(root.path(), "src/lib.rs", "agent-a").unwrap();
    assert!(list(root.path(), None).unwrap().is_empty());
}

#[test]
fn stale_lease_cannot_release_replacement_for_same_agent() {
    let root = tempfile::tempdir().unwrap();
    let old = claim(root.path(), "src/lib.rs", "agent-a", None, 60).unwrap();
    let new = claim(root.path(), "src/lib.rs", "agent-a", Some("new-task"), 60).unwrap();
    assert_ne!(old.lease_id, new.lease_id);

    assert!(matches!(
        release_lease(root.path(), "src/lib.rs", "agent-a", &old.lease_id),
        Err(Error::NotOwner { .. })
    ));
    assert_eq!(list(root.path(), None).unwrap(), vec![new.clone()]);

    release_lease(root.path(), "src/lib.rs", "agent-a", &new.lease_id).unwrap();
    assert!(list(root.path(), None).unwrap().is_empty());
}

#[test]
fn rejects_unsafe_paths() {
    let root = tempfile::tempdir().unwrap();
    assert!(matches!(
        claim(root.path(), "../secret", "agent", None, 60),
        Err(Error::InvalidPath(_))
    ));
    assert!(matches!(
        claim(root.path(), "/etc/passwd", "agent", None, 60),
        Err(Error::InvalidPath(_))
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escape() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    assert!(matches!(
        claim(root.path(), "escape/file", "agent", None, 60),
        Err(Error::InvalidPath(_))
    ));
}
