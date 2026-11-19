use super::*;

#[test]
fn shared_messaging_inbox_survives_restart_without_second_effect() {
    use super::*;
    let mut inbox = SharedInbox::new();
    inbox.send("p", "r", RecipientState::Active, "k1", "hello");
    let mut restored = SharedInbox::from_json(&inbox.to_json().unwrap()).unwrap();
    assert_eq!(restored.visible("p", "r"), vec!["hello"]);
    restored.send("p", "r", RecipientState::Active, "k1", "hello");
    assert_eq!(restored.effects(), 1);
}

#[test]
fn shared_messaging_duplicate_delivery_is_one_effect() {
    let mut inbox = SharedInbox::new();
    let first = inbox.send("p", "r", RecipientState::Active, "k1", "hello");
    let second = inbox.send("p", "r", RecipientState::Active, "k1", "hello");
    assert_eq!(first, MessageState::Accepted);
    assert_eq!(second, MessageState::Accepted);
    assert_eq!(inbox.effects(), 1);
    assert_eq!(inbox.visible("p", "r"), vec!["hello"]);
}

#[test]
fn shared_messaging_lost_ack_stays_unknown_until_acknowledged() {
    let mut inbox = SharedInbox::new();
    inbox.send("p", "r", RecipientState::Active, "k1", "hello");
    assert_eq!(
        inbox.mark_ack_lost("p", "r", "k1"),
        Some(MessageState::Unknown)
    );
    let retry = inbox.send("p", "r", RecipientState::Active, "k1", "hello");
    assert_eq!(retry, MessageState::Unknown);
    assert_eq!(inbox.effects(), 1);
    assert_eq!(
        inbox.mark_acknowledged("p", "r", "k1"),
        Some(MessageState::Acknowledged)
    );
}

#[test]
fn shared_messaging_offline_recipient_is_queued_not_read() {
    let mut inbox = SharedInbox::new();
    let state = inbox.send("p", "r", RecipientState::Offline, "k1", "hello");
    assert_eq!(state, MessageState::Queued);
    assert_ne!(state, MessageState::Acknowledged);
}

#[test]
fn shared_messaging_other_project_sees_nothing() {
    let mut inbox = SharedInbox::new();
    inbox.send(
        "project-a",
        "r",
        RecipientState::Active,
        "k1",
        "secret plan",
    );
    assert!(inbox.visible("project-b", "r").is_empty());
    assert_eq!(inbox.visible("project-a", "r"), vec!["secret plan"]);
}
