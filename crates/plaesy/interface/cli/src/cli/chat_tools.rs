//! Read-only tools for `plaesy chat --tools`.
//!
//! The model asks for a tool by writing one fenced block whose info string is `tool`, holding a
//! JSON object with a `tool` name and its arguments. The host runs it, sends the result back as
//! the next message, and repeats until the model answers without a block. The text protocol works
//! with every provider, including the CLI ones that take one prompt.
//!
//! Only `read_file` and `list_dir` exist. Both are confined to the repository root: an absolute
//! path, a `..` component, or a symlink that leaves the root is refused. Output is cut to
//! [`MAX_OUTPUT_BYTES`], and a turn stops after [`MAX_ROUNDS`] calls.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::delegate::{self, DelegateError, Message, Provider, Request, Response};

/// Tool calls one turn may make before the chat stops and reports the limit.
pub const MAX_ROUNDS: usize = 6;

/// Bytes of tool output sent back to the model. Longer output is cut at a character boundary.
pub const MAX_OUTPUT_BYTES: usize = 8_000;

/// Tells the model which tools exist and how to call them. Sent as a system message when tools are on.
pub const PROTOCOL: &str = "Kamu boleh memakai alat baca-saja di repositori ini. Untuk memanggil \
alat, tulis satu blok berikut dan berhenti:\n\
```tool\n\
{\"tool\":\"read_file\",\"path\":\"relatif/ke/root\"}\n\
```\n\
atau {\"tool\":\"list_dir\",\"path\":\".\"}. Path harus relatif terhadap root repositori, tanpa \
`..`. Hasil alat dikirim sebagai pesan berikutnya. Setelah hasilnya cukup, jawab biasa tanpa \
blok alat.";

const OPEN: &str = "```tool";
const CLOSE: &str = "```";

/// Finds the last `tool` block in `text` and parses its JSON. `None` when there is no block or
/// its body is not a JSON object.
#[must_use]
pub fn find_call(text: &str) -> Option<Value> {
    let start = text.rfind(OPEN)? + OPEN.len();
    let body = &text[start..];
    let end = body.find(CLOSE)?;
    let value: Value = serde_json::from_str(body[..end].trim()).ok()?;
    value.is_object().then_some(value)
}

/// Runs one call and returns its result as text. Errors are returned as text too, so the model
/// can read them and carry on.
#[must_use]
pub fn execute(root: &Path, call: &Value) -> String {
    let tool = call.get("tool").and_then(Value::as_str).unwrap_or("");
    let path = call.get("path").and_then(Value::as_str).unwrap_or("");
    let result = match tool {
        "read_file" => read_file(root, path),
        "list_dir" => list_dir(root, path),
        other => Err(format!("alat tidak dikenal: {other}")),
    };
    match result {
        Ok(text) => truncate(&text),
        Err(err) => format!("error: {err}"),
    }
}

/// One-line note shown in the chat when a call runs.
#[must_use]
pub fn describe(call: &Value) -> String {
    let tool = call.get("tool").and_then(Value::as_str).unwrap_or("?");
    let path = call.get("path").and_then(Value::as_str).unwrap_or("");
    format!("[alat] {tool} {path}")
}

/// Resolves `rel` under `root`. Refuses absolute paths, `..`, and anything whose real location
/// lies outside the canonical root.
///
/// # Errors
///
/// A message naming the refused path, or the I/O error when the path does not exist.
pub fn confine(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim();
    let candidate = if rel.is_empty() { "." } else { rel };
    let path = Path::new(candidate);
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(format!("path harus relatif tanpa `..`: {candidate}"));
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("root tidak bisa dibaca: {e}"))?;
    let full = root
        .join(path)
        .canonicalize()
        .map_err(|e| format!("{candidate}: {e}"))?;
    if !full.starts_with(&root) {
        return Err(format!("path di luar root: {candidate}"));
    }
    Ok(full)
}

fn read_file(root: &Path, rel: &str) -> Result<String, String> {
    let full = confine(root, rel)?;
    if full.is_dir() {
        return Err(format!("{rel} adalah folder; pakai list_dir"));
    }
    fs::read_to_string(&full).map_err(|e| format!("{rel}: {e}"))
}

fn list_dir(root: &Path, rel: &str) -> Result<String, String> {
    let full = confine(root, rel)?;
    if !full.is_dir() {
        return Err(format!("{rel} bukan folder"));
    }
    let mut names: Vec<String> = fs::read_dir(&full)
        .map_err(|e| format!("{rel}: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    names.sort();
    Ok(names.join("\n"))
}

fn truncate(text: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text.to_owned();
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[dipotong]", &text[..end])
}

/// Runs the model with tool rounds. `on_note` receives one line per call before its result runs.
/// The returned response is the reply that carries no tool block.
///
/// # Errors
///
/// The provider's error, or a terminal error when the model still asks for tools after
/// [`MAX_ROUNDS`] calls.
pub fn run_turn(
    provider: &dyn Provider,
    messages: &[Message],
    req: &Request,
    root: &Path,
    on_note: &mut dyn FnMut(&str),
) -> Result<Response, DelegateError> {
    let mut conversation = messages.to_vec();
    for _ in 0..MAX_ROUNDS {
        let response = delegate::run_messages(provider, &conversation, req)?;
        let Some(call) = find_call(&response.text) else {
            return Ok(response);
        };
        on_note(&describe(&call));
        let result = execute(root, &call);
        conversation.push(Message::assistant(response.text.clone()));
        conversation.push(Message::user(format!("Hasil alat:\n{result}")));
    }
    Err(DelegateError::terminal(format!(
        "batas {MAX_ROUNDS} panggilan alat tercapai dalam satu giliran"
    )))
}

#[cfg(test)]
#[path = "chat_tools_test.rs"]
mod chat_tools_test;
