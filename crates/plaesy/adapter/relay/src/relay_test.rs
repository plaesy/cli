//! Unit tests for `tests` split out of `relay.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use tempfile::tempdir;

#[test]
fn idempotent_send_and_explicit_ack_preserve_queue_semantics() {
    let mut store = MessageStore::in_memory().unwrap();
    let first = store.send("m-1", "agent-a", "agent-b", "hello").unwrap();
    assert_eq!(
        store.send("m-1", "agent-a", "agent-b", "hello").unwrap(),
        first
    );
    assert!(matches!(
        store.send("m-1", "agent-a", "agent-b", "changed"),
        Err(MessageError::IdempotencyConflict)
    ));
    assert_eq!(store.inbox("agent-b", 10).unwrap().len(), 1);
    assert!(!store.acknowledge("m-1", "agent-a").unwrap());
    assert!(store.acknowledge("m-1", "agent-b").unwrap());
    assert!(!store.acknowledge("m-1", "agent-b").unwrap());
    assert!(store.inbox("agent-b", 10).unwrap().is_empty());
}

#[test]
fn opening_legacy_schema_adds_lease_columns_and_preserves_pending_messages() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("legacy.sqlite");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE relay_messages (
               id TEXT PRIMARY KEY, sender TEXT NOT NULL, recipient TEXT NOT NULL,
               body TEXT NOT NULL, created_at TEXT NOT NULL, acknowledged_at TEXT,
               UNIQUE(sender, id)
             );
             INSERT INTO relay_messages(id,sender,recipient,body,created_at)
             VALUES ('legacy-1','a','b','preserve me','2026-01-01T00:00:00Z');",
        )
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let mut store = MessageStore::open(&path).unwrap();
    let inbox = store.inbox("b", 10).unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].body, "preserve me");
    let claimed = store.claim_next("b", 60).unwrap().unwrap();
    assert_eq!(claimed.id, "legacy-1");
    assert!(claimed.lease_owner.is_some());
}

#[test]
fn claim_excludes_other_consumers_and_ack_requires_current_token() {
    let mut store = MessageStore::in_memory().unwrap();
    store.send("lease-1", "a", "b", "payload").unwrap();
    let first = store.claim_next("b", 60).unwrap().unwrap();
    let first_token = first.lease_owner.clone().unwrap();
    assert!(first.lease_expiry_epoch_ms.is_some());
    assert!(store.claim_next("b", 60).unwrap().is_none());
    assert!(!store.acknowledge("lease-1", "b").unwrap());
    assert!(!store
        .acknowledge_claim("lease-1", "b", "stale-token")
        .unwrap());
    assert!(store
        .acknowledge_claim("lease-1", "b", &first_token)
        .unwrap());
    assert!(store.inbox("b", 10).unwrap().is_empty());
}

#[test]
fn expired_claim_is_reclaimed_with_a_new_token() {
    let mut store = MessageStore::in_memory().unwrap();
    store.send("lease-expire", "a", "b", "payload").unwrap();
    let first = store.claim_next("b", 60).unwrap().unwrap();
    store
        .connection
        .execute(
            "UPDATE relay_messages SET lease_expiry_epoch_ms=?1 WHERE id=?2",
            params![chrono::Utc::now().timestamp_millis() - 1, "lease-expire"],
        )
        .unwrap();
    assert!(!store
        .acknowledge_claim("lease-expire", "b", first.lease_owner.as_deref().unwrap())
        .unwrap());
    assert!(!store.acknowledge("lease-expire", "b").unwrap());
    let next = store.claim_next("b", 60).unwrap().unwrap();
    assert_ne!(first.lease_owner, next.lease_owner);
    assert!(!store.acknowledge("lease-expire", "b").unwrap());
}

#[test]
fn concurrent_acknowledgments_have_exactly_one_winner() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("messages.sqlite");
    MessageStore::open(&path)
        .unwrap()
        .send("ack-race", "a", "b", "payload")
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = MessageStore::open(path).unwrap();
                barrier.wait();
                store.acknowledge("ack-race", "b").unwrap()
            })
        })
        .collect();
    barrier.wait();
    let acknowledgments: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(acknowledgments.iter().filter(|ack| **ack).count(), 1);
    assert!(MessageStore::open(&path)
        .unwrap()
        .inbox("b", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn concurrent_idempotent_retries_create_only_one_message() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("messages.sqlite");
    MessageStore::open(&path).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = MessageStore::open(path).unwrap();
                barrier.wait();
                store.send("same-id", "a", "b", "payload").unwrap()
            })
        })
        .collect();
    barrier.wait();
    let messages: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(messages[0], messages[1]);
    let store = MessageStore::open(&path).unwrap();
    assert_eq!(store.inbox("b", 10).unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn existing_database_exposed_to_other_users_is_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join("shared.sqlite");
    std::fs::File::create(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        MessageStore::open(&path),
        Err(MessageError::InsecurePermissions)
    ));
}

#[cfg(unix)]
#[test]
fn newly_created_database_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join("private.sqlite");
    MessageStore::open(&path).unwrap();
    let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode & 0o077, 0);
}

#[cfg(unix)]
#[test]
fn restrictive_permissions_on_existing_database_are_preserved() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join("private.sqlite");
    std::fs::File::create(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    MessageStore::open(&path).unwrap();
    let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn pending_message_survives_store_reopen() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("messages.sqlite");
    MessageStore::open(&path)
        .unwrap()
        .send("m-2", "a", "b", "offline")
        .unwrap();
    let reopened = MessageStore::open(&path).unwrap();
    assert_eq!(reopened.inbox("b", 10).unwrap()[0].body, "offline");
}

#[test]
fn idempotent_resend_after_ack_keeps_message_acknowledged() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("messages.sqlite");
    {
        let mut store = MessageStore::open(&path).unwrap();
        store.send("m-3", "a", "b", "done").unwrap();
        assert!(store.acknowledge("m-3", "b").unwrap());
    }
    let mut reopened = MessageStore::open(&path).unwrap();
    let resent = reopened.send("m-3", "a", "b", "done").unwrap();
    assert!(resent.acknowledged_at.is_some());
    assert!(reopened.inbox("b", 10).unwrap().is_empty());
}

#[test]
fn invalid_recipient_and_oversized_body_are_rejected() {
    let mut store = MessageStore::in_memory().unwrap();
    assert!(matches!(
        store.send("m", "a", "../b", "hello"),
        Err(MessageError::Invalid(_))
    ));
    assert!(matches!(
        store.send("m", "a", "b", &"x".repeat(MAX_BODY_BYTES + 1)),
        Err(MessageError::Invalid(_))
    ));
}
