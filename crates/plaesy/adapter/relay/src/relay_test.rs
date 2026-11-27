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
    assert!(!store.acknowledge("agent-a", "m-1", "agent-a").unwrap());
    assert!(store.acknowledge("agent-a", "m-1", "agent-b").unwrap());
    assert!(!store.acknowledge("agent-a", "m-1", "agent-b").unwrap());
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
    assert!(!store.acknowledge("a", "lease-1", "b").unwrap());
    assert!(!store
        .acknowledge_claim("a", "lease-1", "b", "stale-token")
        .unwrap());
    assert!(store
        .acknowledge_claim("a", "lease-1", "b", &first_token)
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
        .acknowledge_claim(
            "a",
            "lease-expire",
            "b",
            first.lease_owner.as_deref().unwrap()
        )
        .unwrap());
    assert!(!store.acknowledge("a", "lease-expire", "b").unwrap());
    let next = store.claim_next("b", 60).unwrap().unwrap();
    assert_ne!(first.lease_owner, next.lease_owner);
    assert!(!store.acknowledge("a", "lease-expire", "b").unwrap());
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
                store.acknowledge("a", "ack-race", "b").unwrap()
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
        assert!(store.acknowledge("a", "m-3", "b").unwrap());
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

/// Moves a message's lease into the past so the next claim sees it as expired.
fn expire_lease(store: &MessageStore, id: &str) {
    store
        .connection
        .execute(
            "UPDATE relay_messages SET lease_expiry_epoch_ms=?1 WHERE id=?2",
            params![chrono::Utc::now().timestamp_millis() - 1, id],
        )
        .unwrap();
}

#[test]
fn status_follows_claim_and_acknowledgment() {
    let mut store = MessageStore::in_memory().unwrap();
    store.send("st-1", "a", "b", "payload").unwrap();
    assert_eq!(
        store.find("a", "st-1").unwrap().unwrap().status,
        MessageStatus::Pending
    );
    let claimed = store.claim_next("b", 60).unwrap().unwrap();
    assert_eq!(claimed.status, MessageStatus::Claimed);
    assert_eq!(claimed.attempts, 1);
    let token = claimed.lease_owner.unwrap();
    assert!(store.acknowledge_claim("a", "st-1", "b", &token).unwrap());
    let done = store.find("a", "st-1").unwrap().unwrap();
    assert_eq!(done.status, MessageStatus::Done);
    assert_eq!(done.attempts, 1);
    assert!(done.acknowledged_at.is_some());
}

#[test]
fn released_claims_retry_until_attempt_limit_then_fail() {
    let mut store = MessageStore::in_memory().unwrap();
    store.send("retry-1", "a", "b", "payload").unwrap();
    for attempt in 1..MAX_ATTEMPTS {
        let claimed = store.claim_next("b", 60).unwrap().unwrap();
        assert_eq!(claimed.attempts, attempt);
        let token = claimed.lease_owner.unwrap();
        assert_eq!(
            store
                .release_claim("a", "retry-1", "b", &token, "boom")
                .unwrap(),
            Some(MessageStatus::Pending)
        );
        assert_eq!(
            store.find("a", "retry-1").unwrap().unwrap().status,
            MessageStatus::Pending
        );
    }
    let last = store.claim_next("b", 60).unwrap().unwrap();
    assert_eq!(last.attempts, MAX_ATTEMPTS);
    let token = last.lease_owner.unwrap();
    assert_eq!(
        store
            .release_claim("a", "retry-1", "b", &token, "boom again")
            .unwrap(),
        Some(MessageStatus::Failed)
    );
    let failed = store.find("a", "retry-1").unwrap().unwrap();
    assert_eq!(failed.status, MessageStatus::Failed);
    assert_eq!(failed.last_error.as_deref(), Some("boom again"));
    assert!(store.claim_next("b", 60).unwrap().is_none());
    assert!(store.inbox("b", 10).unwrap().is_empty());
}

#[test]
fn stale_release_token_is_ignored_and_oversized_error_is_rejected() {
    let mut store = MessageStore::in_memory().unwrap();
    store.send("stale-1", "a", "b", "payload").unwrap();
    let claimed = store.claim_next("b", 60).unwrap().unwrap();
    let token = claimed.lease_owner.unwrap();
    assert_eq!(
        store
            .release_claim("a", "stale-1", "b", "other-token", "x")
            .unwrap(),
        None
    );
    assert!(matches!(
        store.release_claim(
            "a",
            "stale-1",
            "b",
            &token,
            &"e".repeat(MAX_ERROR_BYTES + 1)
        ),
        Err(MessageError::Invalid(_))
    ));
    assert_eq!(
        store.find("a", "stale-1").unwrap().unwrap().status,
        MessageStatus::Claimed
    );
}

#[test]
fn expired_final_attempt_becomes_failed_without_release() {
    let mut store = MessageStore::in_memory().unwrap();
    store.send("expire-1", "a", "b", "payload").unwrap();
    for attempt in 1..=MAX_ATTEMPTS {
        let claimed = store.claim_next("b", 60).unwrap().unwrap();
        assert_eq!(claimed.attempts, attempt);
        expire_lease(&store, "expire-1");
        let expected = if attempt < MAX_ATTEMPTS {
            MessageStatus::Pending
        } else {
            MessageStatus::Failed
        };
        assert_eq!(
            store.find("a", "expire-1").unwrap().unwrap().status,
            expected
        );
    }
    assert!(store.claim_next("b", 60).unwrap().is_none());
    assert!(store.inbox("b", 10).unwrap().is_empty());
}

#[test]
fn purge_removes_only_done_messages_older_than_limit() {
    let mut store = MessageStore::in_memory().unwrap();
    store.send("old-done", "a", "b", "x").unwrap();
    store.send("new-done", "a", "b", "y").unwrap();
    store.send("pending", "a", "b", "z").unwrap();
    assert!(store.acknowledge("a", "old-done", "b").unwrap());
    assert!(store.acknowledge("a", "new-done", "b").unwrap());
    let three_days_ms = 3 * 86_400_000_i64;
    store
        .connection
        .execute(
            "UPDATE relay_messages SET acked_at_epoch_ms=?1 WHERE id='old-done'",
            params![chrono::Utc::now().timestamp_millis() - three_days_ms],
        )
        .unwrap();
    assert_eq!(
        store
            .purge_completed(std::time::Duration::from_secs(86_400))
            .unwrap(),
        1
    );
    assert!(store.find("a", "old-done").unwrap().is_none());
    assert_eq!(
        store.find("a", "new-done").unwrap().unwrap().status,
        MessageStatus::Done
    );
    assert_eq!(
        store.find("a", "pending").unwrap().unwrap().status,
        MessageStatus::Pending
    );
}

#[test]
fn opening_database_with_lease_schema_migrates_attempts_and_ack_times() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lease-era.sqlite");
    let future_ms = chrono::Utc::now().timestamp_millis() + 60_000;
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(&format!(
            "CREATE TABLE relay_messages (
               id TEXT PRIMARY KEY, sender TEXT NOT NULL, recipient TEXT NOT NULL,
               body TEXT NOT NULL, created_at TEXT NOT NULL, acknowledged_at TEXT,
               lease_owner TEXT, lease_expiry_epoch_ms INTEGER, UNIQUE(sender, id)
             );
             INSERT INTO relay_messages(id,sender,recipient,body,created_at,acknowledged_at)
             VALUES ('old-acked','a','b','x','2000-01-01T00:00:00Z','2000-01-02T00:00:00Z');
             INSERT INTO relay_messages(id,sender,recipient,body,created_at,lease_owner,lease_expiry_epoch_ms)
             VALUES ('old-leased','a','b','y','2000-01-01T00:00:01Z','old-token',{future_ms});"
        ))
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let mut store = MessageStore::open(&path).unwrap();
    assert_eq!(
        store.find("a", "old-acked").unwrap().unwrap().status,
        MessageStatus::Done
    );
    let leased = store.find("a", "old-leased").unwrap().unwrap();
    assert_eq!(leased.status, MessageStatus::Claimed);
    assert_eq!(leased.attempts, 1);
    assert!(store
        .acknowledge_claim("a", "old-leased", "b", "old-token")
        .unwrap());
    // The legacy RFC 3339 acknowledgment time was converted, so only the old row is purged.
    assert_eq!(
        store
            .purge_completed(std::time::Duration::from_secs(3600))
            .unwrap(),
        1
    );
    assert!(store.find("a", "old-acked").unwrap().is_none());
    assert!(store.find("a", "old-leased").unwrap().is_some());
}

#[test]
fn task_moves_only_along_allowed_transitions() {
    let mut store = MessageStore::in_memory().expect("store");
    assert_eq!(
        store.create_task("t-1", "alpha", "beta").expect("create"),
        TaskState::Submitted
    );
    assert_eq!(
        store
            .set_task_state("alpha", "t-1", TaskState::Working)
            .expect("start"),
        TaskState::Working
    );
    assert!(matches!(
        store.set_task_state("alpha", "t-1", TaskState::Submitted),
        Err(MessageError::TaskTransition(_))
    ));
    store
        .set_task_state("alpha", "t-1", TaskState::Completed)
        .expect("finish");
    assert!(matches!(
        store.set_task_state("alpha", "t-1", TaskState::Working),
        Err(MessageError::TaskTransition(_))
    ));
    assert_eq!(
        store.task_state("alpha", "t-1").expect("read"),
        Some(TaskState::Completed)
    );
}

#[test]
fn task_ids_are_scoped_to_sender_and_cannot_be_reopened() {
    let mut store = MessageStore::in_memory().expect("store");
    store.create_task("t-2", "alpha", "beta").expect("create");
    assert!(store.create_task("t-2", "alpha", "beta").is_err());
    assert_eq!(store.task_state("gamma", "t-2").expect("read"), None);
    assert!(matches!(
        store.set_task_state("gamma", "t-2", TaskState::Working),
        Err(MessageError::TaskNotFound(_))
    ));
}

#[test]
fn send_task_claim_and_ack_walk_submitted_working_completed() {
    let mut store = MessageStore::in_memory().expect("store");
    assert_eq!(
        store.send_task("k-1", "alpha", "beta", "hi").expect("send"),
        TaskState::Submitted
    );
    let claimed = store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    assert_eq!(
        store.task_state("alpha", "k-1").expect("read"),
        Some(TaskState::Working)
    );
    let token = claimed.lease_owner.clone().expect("lease");
    assert!(store
        .acknowledge_claim("alpha", "k-1", "beta", &token)
        .expect("ack"));
    assert_eq!(
        store.task_state("alpha", "k-1").expect("read"),
        Some(TaskState::Completed)
    );
}

#[test]
fn legacy_ack_of_never_claimed_task_completes_it_through_working() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("k-2", "alpha", "beta", "hi").expect("send");
    assert!(store.acknowledge("alpha", "k-2", "beta").expect("ack"));
    assert_eq!(
        store.task_state("alpha", "k-2").expect("read"),
        Some(TaskState::Completed)
    );
}

#[test]
fn non_final_release_keeps_working_and_final_release_fails_the_task() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("k-3", "alpha", "beta", "hi").expect("send");
    let mut status = None;
    for _ in 0..MAX_ATTEMPTS {
        let claimed = store
            .claim_next("beta", 60)
            .expect("claim")
            .expect("message");
        let token = claimed.lease_owner.clone().expect("lease");
        status = store
            .release_claim("alpha", "k-3", "beta", &token, "boom")
            .expect("release");
        if status == Some(MessageStatus::Pending) {
            assert_eq!(
                store.task_state("alpha", "k-3").expect("read"),
                Some(TaskState::Working)
            );
        }
    }
    assert_eq!(status, Some(MessageStatus::Failed));
    assert_eq!(
        store.task_state("alpha", "k-3").expect("read"),
        Some(TaskState::Failed)
    );
}

#[test]
fn canceled_task_closes_its_message_and_refuses_the_held_claim() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("k-4", "alpha", "beta", "hi").expect("send");
    let claimed = store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    let token = claimed.lease_owner.clone().expect("lease");
    store
        .set_task_state("alpha", "k-4", TaskState::Canceled)
        .expect("cancel");
    // The message is closed as failed with its reason, and it is no longer claimable.
    let closed = store.find("alpha", "k-4").expect("find").expect("message");
    assert_eq!(closed.status, MessageStatus::Failed);
    assert_eq!(closed.last_error.as_deref(), Some("task canceled"));
    assert!(store.claim_next("beta", 60).expect("claim").is_none());
    // The held lease is refused because the task is no longer working; the task stays canceled.
    assert!(matches!(
        store.acknowledge_claim("alpha", "k-4", "beta", &token),
        Err(MessageError::TaskTransition(_))
    ));
    assert_eq!(
        store.task_state("alpha", "k-4").expect("read"),
        Some(TaskState::Canceled)
    );
}

#[test]
fn message_without_task_still_claims_and_acks() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send("k-5", "alpha", "beta", "plain").expect("send");
    let claimed = store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    let token = claimed.lease_owner.clone().expect("lease");
    assert!(store
        .acknowledge_claim("alpha", "k-5", "beta", &token)
        .expect("ack"));
    assert_eq!(store.task_state("alpha", "k-5").expect("read"), None);
}

#[test]
fn send_task_replay_is_idempotent_and_conflicting_content_is_rejected() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("k-6", "alpha", "beta", "hi").expect("send");
    assert_eq!(
        store
            .send_task("k-6", "alpha", "beta", "hi")
            .expect("replay"),
        TaskState::Submitted
    );
    assert_eq!(store.inbox("beta", 10).expect("inbox").len(), 1);
    assert!(matches!(
        store.send_task("k-6", "alpha", "beta", "changed"),
        Err(MessageError::IdempotencyConflict)
    ));
    assert!(matches!(
        store.send_task("k-6", "alpha", "gamma", "hi"),
        Err(MessageError::IdempotencyConflict)
    ));
}

#[test]
fn send_task_reuses_an_orphan_task_row_only_when_it_is_still_submitted_to_the_same_recipient() {
    let mut store = MessageStore::in_memory().expect("store");
    store.create_task("k-7", "alpha", "beta").expect("orphan");
    assert_eq!(
        store.send_task("k-7", "alpha", "beta", "hi").expect("send"),
        TaskState::Submitted
    );
    assert_eq!(store.inbox("beta", 10).expect("inbox").len(), 1);

    store.create_task("k-8", "alpha", "beta").expect("orphan");
    assert!(matches!(
        store.send_task("k-8", "alpha", "gamma", "hi"),
        Err(MessageError::IdempotencyConflict)
    ));
    assert!(store.inbox("gamma", 10).expect("inbox").is_empty());
}

/// Claims the next message for `beta` and returns its lease token.
fn claim_for_beta(store: &mut MessageStore) -> String {
    store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message")
        .lease_owner
        .expect("lease")
}

#[test]
fn request_input_parks_the_message_until_the_sender_answers() {
    let mut store = MessageStore::in_memory().expect("store");
    store
        .send_task("q-1", "alpha", "beta", "plan it")
        .expect("send");
    let token = claim_for_beta(&mut store);
    store
        .request_input("alpha", "q-1", "beta", &token, "which branch?")
        .expect("request input");

    assert_eq!(
        store.task_state("alpha", "q-1").expect("read"),
        Some(TaskState::InputRequired)
    );
    let view = store
        .task_view("alpha", "q-1")
        .expect("view")
        .expect("task");
    assert_eq!(view.state, TaskState::InputRequired);
    assert_eq!(view.prompt.as_deref(), Some("which branch?"));
    let parked = store.find("alpha", "q-1").expect("find").expect("message");
    assert_eq!(parked.status, MessageStatus::AwaitingInput);
    assert_eq!(parked.lease_owner, None);
    // A question is not a failed attempt, so the attempt is refunded.
    assert_eq!(parked.attempts, 0);
    // A parked message is not claimable, and neither ack path accepts it.
    assert!(store.claim_next("beta", 60).expect("claim").is_none());
    assert!(!store
        .acknowledge("alpha", "q-1", "beta")
        .expect("legacy ack"));
    // A parked task is not `working`, so the claim ack is refused with `TaskTransition`.
    assert!(matches!(
        store.acknowledge_claim("alpha", "q-1", "beta", &token),
        Err(MessageError::TaskTransition(_))
    ));
}

#[test]
fn provide_input_resumes_the_task_and_the_answer_arrives_with_the_next_claim() {
    let mut store = MessageStore::in_memory().expect("store");
    store
        .send_task("q-2", "alpha", "beta", "plan it")
        .expect("send");
    let token = claim_for_beta(&mut store);
    store
        .request_input("alpha", "q-2", "beta", &token, "which branch?")
        .expect("ask");

    assert_eq!(
        store
            .provide_input("alpha", "q-2", "use main")
            .expect("answer"),
        TaskState::Working
    );
    assert_eq!(
        store.task_state("alpha", "q-2").expect("read"),
        Some(TaskState::Working)
    );
    assert!(store
        .task_view("alpha", "q-2")
        .expect("view")
        .expect("task")
        .prompt
        .is_none());

    let resumed = store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    assert_eq!(resumed.input_reply.as_deref(), Some("use main"));
    assert_eq!(resumed.attempts, 1);
    let token = resumed.lease_owner.expect("lease");
    assert!(store
        .acknowledge_claim("alpha", "q-2", "beta", &token)
        .expect("ack"));
    assert_eq!(
        store.task_state("alpha", "q-2").expect("read"),
        Some(TaskState::Completed)
    );
}

#[test]
fn request_input_needs_the_live_lease_and_changes_nothing_on_refusal() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("q-3", "alpha", "beta", "hi").expect("send");
    let token = claim_for_beta(&mut store);
    assert!(matches!(
        store.request_input("alpha", "q-3", "beta", "wrong-token", "why?"),
        Err(MessageError::LeaseConflict)
    ));
    assert!(matches!(
        store.request_input("alpha", "q-3", "gamma", &token, "why?"),
        Err(MessageError::LeaseConflict)
    ));
    assert_eq!(
        store.task_state("alpha", "q-3").expect("read"),
        Some(TaskState::Working)
    );
    store
        .request_input("alpha", "q-3", "beta", &token, "why?")
        .expect("ask");
}

#[test]
fn request_input_is_refused_for_a_canceled_task_and_the_message_stays_closed() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("q-4", "alpha", "beta", "hi").expect("send");
    let token = claim_for_beta(&mut store);
    store
        .set_task_state("alpha", "q-4", TaskState::Canceled)
        .expect("cancel");
    assert!(matches!(
        store.request_input("alpha", "q-4", "beta", &token, "why?"),
        Err(MessageError::TaskTransition(_))
    ));
    assert_eq!(
        store.task_state("alpha", "q-4").expect("read"),
        Some(TaskState::Canceled)
    );
    // Cancellation closed the message, so the held token no longer acknowledges it.
    assert!(matches!(
        store.acknowledge_claim("alpha", "q-4", "beta", &token),
        Err(MessageError::TaskTransition(_))
    ));
}

#[test]
fn provide_input_is_refused_unless_the_task_is_input_required() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("q-5", "alpha", "beta", "hi").expect("send");
    assert!(matches!(
        store.provide_input("alpha", "q-5", "early"),
        Err(MessageError::TaskTransition(_))
    ));
    assert!(matches!(
        store.provide_input("alpha", "missing", "x"),
        Err(MessageError::TaskNotFound(_))
    ));
    assert!(matches!(
        store.provide_input("alpha", "q-5", ""),
        Err(MessageError::Invalid(_))
    ));
    let token = claim_for_beta(&mut store);
    store
        .request_input("alpha", "q-5", "beta", &token, "ok?")
        .expect("ask");
    store
        .set_task_state("alpha", "q-5", TaskState::Canceled)
        .expect("cancel");
    assert!(matches!(
        store.provide_input("alpha", "q-5", "late"),
        Err(MessageError::TaskTransition(_))
    ));
}

#[test]
fn canceling_an_input_required_task_closes_the_message_and_drops_the_question() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("q-6", "alpha", "beta", "hi").expect("send");
    let token = claim_for_beta(&mut store);
    store
        .request_input("alpha", "q-6", "beta", &token, "ok?")
        .expect("ask");
    assert!(matches!(
        store.set_task_state("alpha", "q-6", TaskState::Working),
        Err(MessageError::TaskTransition(_))
    ));
    store
        .set_task_state("alpha", "q-6", TaskState::Canceled)
        .expect("cancel");
    // Cancellation closes the message: it is failed, not claimable, and the question is dropped.
    let closed = store.find("alpha", "q-6").expect("find").expect("message");
    assert_eq!(closed.status, MessageStatus::Failed);
    assert_eq!(closed.input_prompt, None);
    assert!(store.claim_next("beta", 60).expect("claim").is_none());
    assert_eq!(
        store.task_state("alpha", "q-6").expect("read"),
        Some(TaskState::Canceled)
    );
}

#[test]
fn parked_message_stays_parked_and_unclaimable() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("q-7", "alpha", "beta", "hi").expect("send");
    let token = claim_for_beta(&mut store);
    store
        .request_input("alpha", "q-7", "beta", &token, "ok?")
        .expect("ask");
    assert!(store.claim_next("beta", 60).expect("claim").is_none());
    assert_eq!(
        store.task_state("alpha", "q-7").expect("read"),
        Some(TaskState::InputRequired)
    );
    assert_eq!(
        store
            .find("alpha", "q-7")
            .expect("find")
            .expect("message")
            .status,
        MessageStatus::AwaitingInput
    );
}

#[test]
fn legacy_schema_message_without_a_task_refuses_input_and_keeps_its_lease() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lease-era-no-input.sqlite");
    let future_ms = chrono::Utc::now().timestamp_millis() + 60_000;
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(&format!(
            "CREATE TABLE relay_messages (
               id TEXT PRIMARY KEY, sender TEXT NOT NULL, recipient TEXT NOT NULL,
               body TEXT NOT NULL, created_at TEXT NOT NULL, acknowledged_at TEXT,
               lease_owner TEXT, lease_expiry_epoch_ms INTEGER, UNIQUE(sender, id)
             );
             INSERT INTO relay_messages(id,sender,recipient,body,created_at,lease_owner,lease_expiry_epoch_ms)
             VALUES ('old-leased','a','b','y','2000-01-01T00:00:01Z','old-token',{future_ms});"
        ))
        .unwrap();
    drop(connection);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let mut store = MessageStore::open(&path).unwrap();
    // No task row exists for this message, so the request is refused and the lease is kept.
    assert!(matches!(
        store.request_input("a", "old-leased", "b", "old-token", "why?"),
        Err(MessageError::TaskNotFound(_))
    ));
    assert!(store
        .acknowledge_claim("a", "old-leased", "b", "old-token")
        .expect("ack"));
}

#[test]
fn message_without_task_row_is_claimable_and_its_replay_reports_lease_conflict() {
    // P1(b): a message sent without a task (through `send`) keeps working through the LEFT JOIN.
    let mut store = MessageStore::in_memory().expect("store");
    store.send("p-1", "alpha", "beta", "plain").expect("send");
    let token = claim_for_beta(&mut store);
    assert!(store
        .acknowledge_claim("alpha", "p-1", "beta", &token)
        .expect("ack"));
    // Replaying the completion without a task row has no state to refuse, so the lease decides.
    assert!(!store
        .acknowledge_claim("alpha", "p-1", "beta", &token)
        .expect("replay"));
}

#[test]
fn completed_task_replay_is_refused_with_task_transition() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("p-2", "alpha", "beta", "hi").expect("send");
    let token = claim_for_beta(&mut store);
    assert!(store
        .acknowledge_claim("alpha", "p-2", "beta", &token)
        .expect("ack"));
    assert!(matches!(
        store.acknowledge_claim("alpha", "p-2", "beta", &token),
        Err(MessageError::TaskTransition(_))
    ));
}

#[test]
fn non_final_release_replay_is_refused_as_lease_conflict() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("p-3", "alpha", "beta", "hi").expect("send");
    let token = claim_for_beta(&mut store);
    assert_eq!(
        store
            .release_claim("alpha", "p-3", "beta", &token, "boom")
            .expect("release"),
        Some(MessageStatus::Pending)
    );
    // The task is still working, so the replay fails on the lease, not on the state.
    assert_eq!(
        store
            .release_claim("alpha", "p-3", "beta", &token, "boom")
            .expect("replay"),
        None
    );
}

#[test]
fn other_recipient_learns_nothing_about_the_task_state() {
    let mut store = MessageStore::in_memory().expect("store");
    store.send_task("p-4", "alpha", "beta", "hi").expect("send");
    let token = claim_for_beta(&mut store);
    store
        .set_task_state("alpha", "p-4", TaskState::Canceled)
        .expect("cancel");
    // `gamma` is not the recipient: the refusal is `LeaseConflict`, never a state-revealing error.
    assert!(matches!(
        store.request_input("alpha", "p-4", "gamma", &token, "why?"),
        Err(MessageError::LeaseConflict)
    ));
    assert!(matches!(
        store.acknowledge_claim("alpha", "p-4", "gamma", &token),
        Ok(false)
    ));
    assert!(matches!(
        store.release_claim("alpha", "p-4", "gamma", &token, "x"),
        Ok(None)
    ));
}

#[test]
fn same_id_from_two_senders_is_two_messages() {
    let mut store = MessageStore::in_memory().expect("store");
    store
        .send("dup-1", "alpha", "beta", "from alpha")
        .expect("alpha");
    // Another sender may use the same id without seeing a conflict, and the two stay separate.
    let other = store
        .send("dup-1", "gamma", "beta", "from gamma")
        .expect("gamma");
    assert_eq!(other.sender, "gamma");
    assert_eq!(
        store
            .find("alpha", "dup-1")
            .expect("find")
            .expect("alpha")
            .body,
        "from alpha"
    );
    assert_eq!(
        store
            .find("gamma", "dup-1")
            .expect("find")
            .expect("gamma")
            .body,
        "from gamma"
    );
}

#[test]
fn opening_a_database_with_the_old_id_key_moves_to_sender_and_id_keys() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("old-key.sqlite");
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE relay_messages (
                   id TEXT PRIMARY KEY, sender TEXT NOT NULL, recipient TEXT NOT NULL,
                   body TEXT NOT NULL, created_at TEXT NOT NULL, acknowledged_at TEXT,
                   UNIQUE(sender, id)
                 );
                 CREATE INDEX relay_messages_inbox
                   ON relay_messages(recipient, acknowledged_at, created_at);
                 INSERT INTO relay_messages(id,sender,recipient,body,created_at)
                   VALUES ('old-1','alpha','beta','kept','2000-01-01T00:00:00Z');",
            )
            .unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let store = MessageStore::open(&path).unwrap();
    // Rows survive the rebuild, and the inbox index is recreated.
    assert_eq!(store.find("alpha", "old-1").unwrap().unwrap().body, "kept");
    let index_count: i64 = store
        .connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='relay_messages_inbox'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(index_count, 1);
    // The new key allows the same id from another sender.
    drop(store);
    let mut store = MessageStore::open(&path).unwrap();
    store.send("old-1", "gamma", "beta", "new").unwrap();
}
