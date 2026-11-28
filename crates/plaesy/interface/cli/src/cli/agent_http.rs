//! HTTP routes for the in-house agent protocol, as pure functions over the relay store.
//!
//! The socket layer is separate, so each route can be tested without a network. Routes:
//!
//! - `POST /message:send` with `{"id","from","to","text"}`, or with `"parts"` instead of `text`,
//!   opens a task and queues the message.
//! - `GET /tasks/{from}/{id}` returns the task state.
//! - `POST /tasks/{from}/{id}:cancel` moves the task to `canceled` when the state allows it.
//! - `POST /tasks/{from}/{id}:input` with `{"text"}` answers a task that is `input-required`.
//! - `GET /.well-known/agent.json` returns the Agent Card: name, description, version, capabilities, skills.
//!
//! A worker asks for input through the store (`MessageStore::request_input`), which needs the lease
//! token from a claim. No HTTP route raises `input-required`, because claims are not exposed over HTTP.
//!
//! Replies are `(status, json body)`. Nothing here reads files or environment variables; the card's
//! version comes from the build-time `CARGO_PKG_VERSION`.

use plaesy_adapter_relay::relay::{MessageError, MessageStore, TaskView};
use plaesy_adapter_relay::task::TaskState;
use serde_json::{json, Value};

/// Largest accepted request body, in bytes.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
/// Most parts one message may carry.
pub const MAX_PARTS: usize = 16;

/// Routes one request. Returns the HTTP status and a JSON body.
pub fn route(store: &mut MessageStore, method: &str, path: &str, body: &[u8]) -> (u16, String) {
    if body.len() > MAX_BODY_BYTES {
        return error(413, "request body is too large");
    }
    match (method, path) {
        ("POST", "/message:send") => send_message(store, body),
        ("GET", "/.well-known/agent.json") => agent_card(),
        ("GET", _) => match parse_task_path(path) {
            Some((sender, id)) => get_task(store, &sender, &id),
            None => error(404, "not found"),
        },
        ("POST", _) => {
            if let Some((sender, id)) = parse_action_path(path, ":cancel") {
                cancel_task(store, &sender, &id)
            } else if let Some((sender, id)) = parse_action_path(path, ":input") {
                answer_task(store, &sender, &id, body)
            } else {
                error(404, "not found")
            }
        }
        _ => error(405, "method not allowed"),
    }
}

/// The Agent Card. Static except for the version. It names only what the protocol does today, and
/// it never includes the bearer token, the database path, or the repository path.
fn agent_card() -> (u16, String) {
    let card = json!({
        "name": "plaesy",
        "description": "Agent-to-agent task protocol of a local Plaesy repository. Messages are queued in a local relay.",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": {
            "streaming": true,
            "cancel": true,
            "idempotentSend": true,
            "pushNotifications": false,
            "inputRequired": true,
            "artifacts": false
        },
        "skills": [
            {
                "id": "message.send",
                "name": "Send message",
                "description": "Open a task and queue a text message for another agent. POST /message:send.",
                "inputModes": ["text", "data"],
                "outputModes": ["text"]
            },
            {
                "id": "task.status",
                "name": "Read task state",
                "description": "Read a task's state, or stream state changes as SSE. GET /tasks/{from}/{id} and /tasks/{from}/{id}/events.",
                "inputModes": [],
                "outputModes": ["application/json", "text/event-stream"]
            },
            {
                "id": "task.cancel",
                "name": "Cancel task",
                "description": "Move a task to canceled when its state allows it. POST /tasks/{from}/{id}:cancel.",
                "inputModes": [],
                "outputModes": ["application/json"]
            },
            {
                "id": "task.input",
                "name": "Answer input request",
                "description": "Answer a task that is input-required, which resumes it. POST /tasks/{from}/{id}:input with {\"text\"}.",
                "inputModes": ["text"],
                "outputModes": ["application/json"]
            }
        ]
    });
    (200, card.to_string())
}

fn send_message(store: &mut MessageStore, body: &[u8]) -> (u16, String) {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return error(400, "body must be JSON");
    };
    let field = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let (Some(id), Some(from), Some(to)) = (field("id"), field("from"), field("to")) else {
        return error(400, "id, from, and to are required strings");
    };
    let body_json = match message_body(&value) {
        Ok(body_json) => body_json,
        Err(reason) => return error(400, &reason),
    };
    // One transaction opens the task and queues the message. A repeat with the same content returns
    // the task's current state, not an error.
    match store.send_task(&id, &from, &to, &body_json) {
        Ok(state) => (
            201,
            json!({ "id": id, "from": from, "to": to, "state": state.as_str() }).to_string(),
        ),
        Err(err) => map_error(err),
    }
}

/// The stored envelope for a send. A `text` send stores exactly the bytes it always has, so an
/// idempotent resend across an upgrade still matches. A `parts` send stores the joined text under
/// `text` as well, so readers that only know the envelope still see the message.
fn message_body(value: &Value) -> Result<String, String> {
    match (value.get("text"), value.get("parts")) {
        (Some(_), Some(_)) => Err("send either text or parts, not both".to_owned()),
        (Some(text), None) => {
            let text = text
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .ok_or_else(|| "text must be a non-empty string".to_owned())?;
            Ok(json!({ "depth": 0, "text": text }).to_string())
        }
        (None, Some(parts)) => {
            let (text, parts) = parse_parts(parts)?;
            Ok(json!({ "depth": 0, "text": text, "parts": parts }).to_string())
        }
        (None, None) => Err("either text or parts is required".to_owned()),
    }
}

/// Validates `parts` and returns the joined text of its text parts, plus the parts in canonical
/// form. Each part is `{"kind":"text","text":"..."}` or `{"kind":"data","data":{...}}`. A message
/// needs at least one non-blank text part, so every reader can show it.
fn parse_parts(parts: &Value) -> Result<(String, Value), String> {
    let items = parts
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| "parts must be a non-empty array".to_owned())?;
    if items.len() > MAX_PARTS {
        return Err(format!("parts may hold at most {MAX_PARTS} items"));
    }
    let mut texts = Vec::new();
    let mut canonical = Vec::with_capacity(items.len());
    for item in items {
        match item.get("kind").and_then(Value::as_str) {
            Some("text") => {
                let text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "a text part needs a string text".to_owned())?;
                texts.push(text);
                canonical.push(json!({ "kind": "text", "text": text }));
            }
            Some("data") => {
                let data = item
                    .get("data")
                    .filter(|data| data.is_object())
                    .ok_or_else(|| "a data part needs an object data".to_owned())?;
                canonical.push(json!({ "kind": "data", "data": canonical_json(data) }));
            }
            _ => return Err("part kind must be text or data".to_owned()),
        }
    }
    let text = texts.join("\n");
    if text.trim().is_empty() {
        return Err("parts need at least one non-empty text part".to_owned());
    }
    Ok((text, Value::Array(canonical)))
}

/// Sorts object keys at every depth. The relay compares stored bodies to detect a changed resend,
/// and this keeps a resend with the same data in a different key order from looking changed.
fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, item)| (key.clone(), canonical_json(item)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn get_task(store: &MessageStore, sender: &str, id: &str) -> (u16, String) {
    match store.task_view(sender, id) {
        Ok(Some(view)) => (200, task_json(id, sender, &view).to_string()),
        Ok(None) => error(404, "no such task"),
        Err(err) => map_error(err),
    }
}

/// Task JSON shared by `GET` and the SSE events. `prompt` appears only while the task is
/// `input-required`, so the sender can read the question.
pub fn task_json(id: &str, sender: &str, view: &TaskView) -> Value {
    let mut value = json!({ "id": id, "from": sender, "state": view.state.as_str() });
    if let Some(prompt) = &view.prompt {
        value["prompt"] = json!(prompt);
    }
    value
}

fn cancel_task(store: &mut MessageStore, sender: &str, id: &str) -> (u16, String) {
    match store.set_task_state(sender, id, TaskState::Canceled) {
        Ok(state) => (
            200,
            json!({ "id": id, "from": sender, "state": state.as_str() }).to_string(),
        ),
        Err(err) => map_error(err),
    }
}

/// `:input` with `{"text"}`. Only a task in `input-required` accepts it; any other state is a 409.
fn answer_task(store: &mut MessageStore, sender: &str, id: &str, body: &[u8]) -> (u16, String) {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return error(400, "body must be JSON");
    };
    let Some(text) = value.get("text").and_then(Value::as_str) else {
        return error(400, "text is required");
    };
    if text.trim().is_empty() {
        return error(400, "text must not be empty");
    }
    match store.provide_input(sender, id, text) {
        Ok(state) => (
            200,
            json!({ "id": id, "from": sender, "state": state.as_str() }).to_string(),
        ),
        Err(err) => map_error(err),
    }
}

/// `/tasks/{from}/{id}` with exactly two non-empty segments.
fn parse_task_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix("/tasks/")?;
    let (sender, id) = rest.split_once('/')?;
    (!sender.is_empty() && !id.is_empty() && !id.contains('/'))
        .then(|| (sender.to_owned(), id.to_owned()))
}

/// `/tasks/{from}/{id}{action}`, for an action such as `:cancel` or `:input`.
fn parse_action_path(path: &str, action: &str) -> Option<(String, String)> {
    let trimmed = path.strip_suffix(action)?;
    parse_task_path(trimmed)
}

fn map_error(err: MessageError) -> (u16, String) {
    match err {
        MessageError::TaskNotFound(_) => error(404, "no such task"),
        MessageError::TaskTransition(message) => error(409, &message),
        MessageError::IdempotencyConflict => error(409, "id already used with different content"),
        MessageError::LeaseConflict => error(409, "message is not leased by this owner"),
        MessageError::Invalid(reason) => error(400, reason),
        _ => error(500, "storage error"),
    }
}

fn error(status: u16, message: &str) -> (u16, String) {
    (status, json!({ "error": message }).to_string())
}

#[cfg(test)]
#[path = "agent_http_test.rs"]
mod agent_http_test;
