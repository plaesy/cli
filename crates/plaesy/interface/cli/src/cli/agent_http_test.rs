use plaesy_adapter_relay::relay::MessageStore;
use serde_json::{json, Value};

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
fn an_identical_resend_is_idempotent_and_changed_content_is_refused() {
    let mut store = MessageStore::in_memory().expect("store");
    assert_eq!(send(&mut store, "t-4").0, 201);
    let (resent, body) = send(&mut store, "t-4");
    assert_eq!(resent, 201);
    assert_eq!(json(&body)["state"], "submitted");
    assert_eq!(store.inbox("beta", 10).expect("inbox").len(), 1);
    let changed = br#"{"id":"t-4","from":"alpha","to":"beta","text":"other"}"#;
    let (conflict, _) = route(&mut store, "POST", "/message:send", changed);
    assert_eq!(conflict, 409);
}

#[test]
fn repeated_send_returns_the_current_state_and_conflicting_content_is_409() {
    let mut store = MessageStore::in_memory().expect("store");
    assert_eq!(send(&mut store, "t-4").0, 201);
    let (status, body) = send(&mut store, "t-4");
    assert_eq!(status, 201);
    assert_eq!(json(&body)["state"], "submitted");
    let changed = br#"{"id":"t-4","from":"alpha","to":"beta","text":"other"}"#;
    let (conflict, _) = route(&mut store, "POST", "/message:send", changed);
    assert_eq!(conflict, 409);
}

#[test]
fn agent_card_lists_required_fields_with_the_package_version() {
    let mut store = MessageStore::in_memory().expect("store");
    let (status, body) = route(&mut store, "GET", "/.well-known/agent.json", b"");
    assert_eq!(status, 200);
    let card = json(&body);
    assert_eq!(card["name"], "plaesy");
    assert!(card["description"]
        .as_str()
        .is_some_and(|text| !text.is_empty()));
    assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(card["capabilities"]["cancel"], true);
    assert_eq!(card["capabilities"]["pushNotifications"], false);
    let skills = card["skills"].as_array().expect("skills array");
    assert!(skills.iter().any(|skill| skill["id"] == "message.send"));
    assert!(skills
        .iter()
        .all(|skill| skill["id"].is_string() && skill["name"].is_string()));
}

#[test]
fn agent_card_omits_secrets_and_local_paths() {
    let mut store = MessageStore::in_memory().expect("store");
    let (_, body) = route(&mut store, "GET", "/.well-known/agent.json", b"");
    assert!(!body.contains("relay.sqlite"));
    assert!(!body.contains(".plaesy"));
    assert!(!body.contains("PLAESY_AGENT_TOKEN"));
    assert!(!body.contains("Bearer"));
    assert!(!body.contains("C:\\"));
    assert!(!body.contains("/Users/"));
    assert!(!body.contains("/home/"));
}

#[test]
fn agent_card_is_get_only() {
    let mut store = MessageStore::in_memory().expect("store");
    let (status, _) = route(&mut store, "POST", "/.well-known/agent.json", b"");
    assert_eq!(status, 404);
}

#[test]
fn send_claim_get_ack_get_walks_the_task_to_completed() {
    let mut store = MessageStore::in_memory().expect("store");
    send(&mut store, "t-5");
    let claimed = store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    let (_, working) = route(&mut store, "GET", "/tasks/alpha/t-5", b"");
    assert_eq!(json(&working)["state"], "working");
    let token = claimed.lease_owner.expect("lease");
    assert!(store.acknowledge_claim("t-5", "beta", &token).expect("ack"));
    let (_, completed) = route(&mut store, "GET", "/tasks/alpha/t-5", b"");
    assert_eq!(json(&completed)["state"], "completed");
}

#[test]
fn a_text_send_stores_the_same_envelope_as_before_parts_existed() {
    let mut store = MessageStore::in_memory().expect("store");
    assert_eq!(send(&mut store, "p-1").0, 201);
    let inbox = store.inbox("beta", 10).expect("inbox");
    assert_eq!(inbox[0].body, r#"{"depth":0,"text":"halo"}"#);
}

#[test]
fn a_parts_send_queues_the_joined_text_and_keeps_the_parts() {
    let mut store = MessageStore::in_memory().expect("store");
    let body = br#"{"id":"p-2","from":"alpha","to":"beta","parts":[
        {"kind":"text","text":"first"},
        {"kind":"data","data":{"b":1,"a":{"y":2,"x":[3]}}},
        {"kind":"text","text":"second"}]}"#;
    let (status, reply) = route(&mut store, "POST", "/message:send", body);
    assert_eq!(status, 201);
    assert_eq!(json(&reply)["state"], "submitted");
    let inbox = store.inbox("beta", 10).expect("inbox");
    let stored = json(&inbox[0].body);
    assert_eq!(stored["depth"], 0);
    assert_eq!(stored["text"], "first\nsecond");
    assert_eq!(stored["parts"][0], json!({"kind":"text","text":"first"}));
    assert_eq!(
        stored["parts"][1]["data"],
        json!({"a":{"x":[3],"y":2},"b":1})
    );
    assert_eq!(stored["parts"].as_array().map(Vec::len), Some(3));
}

#[test]
fn a_parts_resend_with_reordered_data_keys_is_idempotent() {
    let mut store = MessageStore::in_memory().expect("store");
    let first = br#"{"id":"p-3","from":"alpha","to":"beta","parts":[{"kind":"text","text":"hi"},{"kind":"data","data":{"a":1,"b":2}}]}"#;
    let second = br#"{"id":"p-3","from":"alpha","to":"beta","parts":[{"kind":"text","text":"hi"},{"kind":"data","data":{"b":2,"a":1}}]}"#;
    assert_eq!(route(&mut store, "POST", "/message:send", first).0, 201);
    assert_eq!(route(&mut store, "POST", "/message:send", second).0, 201);
    assert_eq!(store.inbox("beta", 10).expect("inbox").len(), 1);
}

#[test]
fn a_parts_send_is_readable_by_the_chat_envelope_decoder() {
    let mut store = MessageStore::in_memory().expect("store");
    let body = br#"{"id":"p-4","from":"alpha","to":"beta","parts":[{"kind":"text","text":"hello"},{"kind":"data","data":{"k":true}}]}"#;
    assert_eq!(route(&mut store, "POST", "/message:send", body).0, 201);
    let inbox = store.inbox("beta", 10).expect("inbox");
    let envelope = crate::cli::chat_agents::decode(&inbox[0].body);
    assert_eq!(envelope.depth, 0);
    assert_eq!(envelope.text, "hello");
}

#[test]
fn invalid_parts_are_rejected_with_400() {
    let cases: [&[u8]; 9] = [
        br#"{"id":"x","from":"a","to":"b","parts":[]}"#,
        br#"{"id":"x","from":"a","to":"b","parts":"hi"}"#,
        br#"{"id":"x","from":"a","to":"b","parts":[{"kind":"file","file":"x"}]}"#,
        br#"{"id":"x","from":"a","to":"b","parts":[{"kind":"text"}]}"#,
        br#"{"id":"x","from":"a","to":"b","parts":[{"kind":"text","text":"  "}]}"#,
        br#"{"id":"x","from":"a","to":"b","parts":[{"kind":"data","data":"no"}]}"#,
        br#"{"id":"x","from":"a","to":"b","parts":[{"kind":"data","data":{"k":1}}]}"#,
        br#"{"id":"x","from":"a","to":"b","text":"hi","parts":[{"kind":"text","text":"hi"}]}"#,
        br#"{"id":"x","from":"a","to":"b"}"#,
    ];
    let mut store = MessageStore::in_memory().expect("store");
    for body in cases {
        let (status, _) = route(&mut store, "POST", "/message:send", body);
        assert_eq!(status, 400, "{}", String::from_utf8_lossy(body));
    }
    let too_many: Vec<Value> = (0..=super::MAX_PARTS)
        .map(|_| json!({"kind": "text", "text": "a"}))
        .collect();
    let body = json!({"id": "x", "from": "a", "to": "b", "parts": too_many}).to_string();
    assert_eq!(
        route(&mut store, "POST", "/message:send", body.as_bytes()).0,
        400
    );
}

#[test]
fn a_parts_send_past_the_stored_body_limit_is_400_not_500() {
    let mut store = MessageStore::in_memory().expect("store");
    // The text is stored twice (as `text` and inside `parts`), so this fits the request
    // limit but not the stored envelope.
    let text = "a".repeat(40 * 1024);
    let body = json!({
        "id": "big",
        "from": "alpha",
        "to": "beta",
        "parts": [{"kind": "text", "text": text}]
    })
    .to_string();
    assert!(body.len() <= super::MAX_BODY_BYTES);
    let (status, _) = route(&mut store, "POST", "/message:send", body.as_bytes());
    assert_eq!(status, 400);
}
