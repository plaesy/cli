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
fn input_round_trip_shows_the_question_and_resumes_on_answer() {
    let mut store = MessageStore::in_memory().expect("store");
    send(&mut store, "t-in-1");
    let claimed = store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    let token = claimed.lease_owner.expect("lease");
    store
        .request_input("t-in-1", "beta", &token, "which branch?")
        .expect("ask");

    let (status, body) = route(&mut store, "GET", "/tasks/alpha/t-in-1", b"");
    assert_eq!(status, 200);
    assert_eq!(json(&body)["state"], "input-required");
    assert_eq!(json(&body)["prompt"], "which branch?");

    let (answered, body) = route(
        &mut store,
        "POST",
        "/tasks/alpha/t-in-1:input",
        br#"{"text":"use main"}"#,
    );
    assert_eq!(answered, 200);
    assert_eq!(json(&body)["state"], "working");

    let (_, body) = route(&mut store, "GET", "/tasks/alpha/t-in-1", b"");
    assert!(json(&body).get("prompt").is_none(), "{body}");
    let resumed = store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    assert_eq!(resumed.input_reply.as_deref(), Some("use main"));
}

#[test]
fn input_route_refuses_wrong_state_missing_text_and_unknown_tasks() {
    let mut store = MessageStore::in_memory().expect("store");
    send(&mut store, "t-in-2");
    let (not_waiting, _) = route(
        &mut store,
        "POST",
        "/tasks/alpha/t-in-2:input",
        br#"{"text":"early"}"#,
    );
    assert_eq!(not_waiting, 409);
    let (missing_text, _) = route(&mut store, "POST", "/tasks/alpha/t-in-2:input", b"{}");
    assert_eq!(missing_text, 400);
    let (blank, _) = route(
        &mut store,
        "POST",
        "/tasks/alpha/t-in-2:input",
        br#"{"text":"   "}"#,
    );
    assert_eq!(blank, 400);
    let (not_json, _) = route(&mut store, "POST", "/tasks/alpha/t-in-2:input", b"nope");
    assert_eq!(not_json, 400);
    let (unknown, _) = route(
        &mut store,
        "POST",
        "/tasks/alpha/nope:input",
        br#"{"text":"x"}"#,
    );
    assert_eq!(unknown, 404);
}

#[test]
fn stale_lease_on_request_input_maps_to_conflict() {
    let mut store = MessageStore::in_memory().expect("store");
    send(&mut store, "t-in-3");
    store
        .claim_next("beta", 60)
        .expect("claim")
        .expect("message");
    let refused = store.request_input("t-in-3", "beta", "stale", "why?");
    let (status, _) = match refused {
        Err(err) => super::map_error(err),
        Ok(()) => panic!("stale lease must not park the task"),
    };
    assert_eq!(status, 409);
}

#[test]
fn agent_card_advertises_input_required_and_the_answer_skill() {
    let mut store = MessageStore::in_memory().expect("store");
    let (_, body) = route(&mut store, "GET", "/.well-known/agent.json", b"");
    let card = json(&body);
    assert_eq!(card["capabilities"]["inputRequired"], true);
    let skills = card["skills"].as_array().expect("skills");
    assert!(skills.iter().any(|skill| skill["id"] == "task.input"));
}
