//! HTTP routes for the in-house agent protocol, as pure functions over the relay store.
//!
//! The socket layer is separate, so each route can be tested without a network. Routes:
//!
//! - `POST /message:send` with `{"id","from","to","text"}` opens a task and queues the message.
//! - `GET /tasks/{from}/{id}` returns the task state.
//! - `POST /tasks/{from}/{id}:cancel` moves the task to `canceled` when the state allows it.
//!
//! Replies are `(status, json body)`. Nothing here reads files or environment variables.

use plaesy_adapter_relay::relay::{MessageError, MessageStore};
use plaesy_adapter_relay::task::TaskState;
use serde_json::{json, Value};

/// Largest accepted request body, in bytes.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Routes one request. Returns the HTTP status and a JSON body.
pub fn route(store: &mut MessageStore, method: &str, path: &str, body: &[u8]) -> (u16, String) {
    if body.len() > MAX_BODY_BYTES {
        return error(413, "request body is too large");
    }
    match (method, path) {
        ("POST", "/message:send") => send_message(store, body),
        ("GET", _) => match parse_task_path(path) {
            Some((sender, id)) => get_task(store, &sender, &id),
            None => error(404, "not found"),
        },
        ("POST", _) => match parse_cancel_path(path) {
            Some((sender, id)) => cancel_task(store, &sender, &id),
            None => error(404, "not found"),
        },
        _ => error(405, "method not allowed"),
    }
}

fn send_message(store: &mut MessageStore, body: &[u8]) -> (u16, String) {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return error(400, "body must be JSON");
    };
    let field = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let (Some(id), Some(from), Some(to), Some(text)) =
        (field("id"), field("from"), field("to"), field("text"))
    else {
        return error(400, "id, from, to, and text are required strings");
    };
    if text.trim().is_empty() {
        return error(400, "text must not be empty");
    }
    let body_json = json!({ "depth": 0, "text": text }).to_string();
    if let Err(err) = store.create_task(&id, &from, &to) {
        return map_error(err);
    }
    match store.send(&id, &from, &to, &body_json) {
        Ok(_) => (
            201,
            json!({ "id": id, "from": from, "to": to, "state": TaskState::Submitted.as_str() })
                .to_string(),
        ),
        Err(err) => map_error(err),
    }
}

fn get_task(store: &MessageStore, sender: &str, id: &str) -> (u16, String) {
    match store.task_state(sender, id) {
        Ok(Some(state)) => (
            200,
            json!({ "id": id, "from": sender, "state": state.as_str() }).to_string(),
        ),
        Ok(None) => error(404, "no such task"),
        Err(err) => map_error(err),
    }
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

/// `/tasks/{from}/{id}` with exactly two non-empty segments.
fn parse_task_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix("/tasks/")?;
    let (sender, id) = rest.split_once('/')?;
    (!sender.is_empty() && !id.is_empty() && !id.contains('/'))
        .then(|| (sender.to_owned(), id.to_owned()))
}

/// `/tasks/{from}/{id}:cancel`.
fn parse_cancel_path(path: &str) -> Option<(String, String)> {
    let trimmed = path.strip_suffix(":cancel")?;
    parse_task_path(trimmed)
}

fn map_error(err: MessageError) -> (u16, String) {
    match err {
        MessageError::TaskNotFound(_) => error(404, "no such task"),
        MessageError::TaskTransition(message) => error(409, &message),
        MessageError::IdempotencyConflict => error(409, "id already used with different content"),
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
