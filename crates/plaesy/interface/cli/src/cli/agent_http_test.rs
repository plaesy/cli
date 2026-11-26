use plaesy_adapter_relay::relay::MessageStore;
use serde_json::Value;

use super::route;

fn json(body: &str) -> Value {
    serde_json::from_str(body).expect("json body")
}

fn send(store: &mut MessageStore, id: &str) -> (u16, String) {
    let body = format!(r#"{{"id":"{id}","from":"alpha","to":"beta","text":"halo"}}"#);
    route(store, "POST", "/message:send", body.as_bytes())
}

#[test]
fn send_opens_a_submitted_task_and_queues_the_message() {
    let mut store = MessageStore::in_memory().expect("store");
    let (status, body) = send(&mut store, "t-1");
    assert_eq!(status, 201);
    assert_eq!(json(&body)["state"], "submitted");
    let inbox = store.inbox("beta", 10).expect("inbox");
    assert_eq!(inbox.len(), 1);
}

#[test]
fn get_returns_the_state_and_unknown_tasks_are_404() {
    let mut store = MessageStore::in_memory().expect("store");
    send(&mut store, "t-2");
    let (status, body) = route(&mut store, "GET", "/tasks/alpha/t-2", b"");
    assert_eq!(status, 200);
    assert_eq!(json(&body)["state"], "submitted");
    let (missing, _) = route(&mut store, "GET", "/tasks/alpha/nope", b"");
    assert_eq!(missing, 404);
}

#[test]
fn cancel_moves_the_task_once_and_refuses_after_a_terminal_state() {
    let mut store = MessageStore::in_memory().expect("store");
    send(&mut store, "t-3");
    let (status, body) = route(&mut store, "POST", "/tasks/alpha/t-3:cancel", b"");
    assert_eq!(status, 200);
    assert_eq!(json(&body)["state"], "canceled");
    let (again, _) = route(&mut store, "POST", "/tasks/alpha/t-3:cancel", b"");
    assert_eq!(again, 409);
}

#[test]
fn malformed_or_oversized_requests_are_rejected() {
    let mut store = MessageStore::in_memory().expect("store");
    let (bad_json, _) = route(&mut store, "POST", "/message:send", b"not json");
    assert_eq!(bad_json, 400);
    let (missing, _) = route(&mut store, "POST", "/message:send", br#"{"id":"x"}"#);
    assert_eq!(missing, 400);
    let (empty, _) = route(
        &mut store,
        "POST",
        "/message:send",
        br#"{"id":"x","from":"a","to":"b","text":"  "}"#,
    );
    assert_eq!(empty, 400);
    let big = vec![b' '; super::MAX_BODY_BYTES + 1];
    let (too_big, _) = route(&mut store, "POST", "/message:send", &big);
    assert_eq!(too_big, 413);
    let (method, _) = route(&mut store, "PUT", "/message:send", b"");
    assert_eq!(method, 405);
}

#[test]
fn a_task_id_cannot_be_sent_twice_from_the_same_sender() {
    let mut store = MessageStore::in_memory().expect("store");
    assert_eq!(send(&mut store, "t-4").0, 201);
    assert_ne!(send(&mut store, "t-4").0, 201);
}
