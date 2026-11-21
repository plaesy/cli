//! Agen ACP v1 lewat stdio (ADR 0003: interface/acp). Spesifikasi:
//! <https://agentclientprotocol.com/protocol/v1/overview>.
//!
//! Subset yang diimplementasikan:
//! - `initialize` (request): menyepakati `protocolVersion` 1 dan mengumumkan kapabilitas agen.
//! - `session/new` (request): membuat sesi dengan `cwd` absolut. Balasan berisi `sessionId`.
//! - `session/prompt` (request): menerima pesan pengguna. Agen mengirim `session/update`
//!   (notifikasi), lalu membalas `stopReason`.
//! - `session/cancel` (notifikasi): tidak dibalas.
//!
//! Plaesy belum punya model bahasa yang terpasang. Karena itu `session/prompt` menjawab
//! dengan pesan yang menyatakan hal itu, lalu `end_turn`. Jawaban palsu tidak dibuat.
//!
//! Operasi dokumen tersedia sebagai ekstensi `_plaesy/document/*`. Nama dengan awalan `_`
//! mengikuti konvensi ekstensi ACP; aturan itu belum saya verifikasi di spesifikasi.
//!
//! Stdout hanya berisi pesan JSON-RPC, satu per baris. Diagnostik tidak ditulis ke stdout.

use plaesy_service::port::{DocumentStore, StoreError};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Mutex;

/// Versi protokol ACP yang didukung (bilangan bulat di spesifikasi v1).
pub const PROTOCOL_VERSION: u64 = 1;

/// Pesan yang dikirim saat `session/prompt`, karena belum ada model terpasang.
pub const NO_MODEL_TEXT: &str =
    "Plaesy belum punya model bahasa terpasang. Pesan diterima, tetapi tidak ada jawaban yang dihasilkan.";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const SERVER_ERROR: i64 = -32000;

type Failure = (i64, String);

/// Agen ACP di atas sebuah penyimpanan dokumen. Menyimpan sesi yang aktif.
pub struct Agent<'a> {
    store: &'a dyn DocumentStore,
    sessions: Mutex<Vec<String>>,
}

impl<'a> Agent<'a> {
    /// Agen baru tanpa sesi.
    #[must_use]
    pub fn new(store: &'a dyn DocumentStore) -> Self {
        Self {
            store,
            sessions: Mutex::new(Vec::new()),
        }
    }

    /// Memproses satu pesan masuk. Mengembalikan notifikasi yang harus dikirim lebih dulu,
    /// lalu balasan (bila pesan itu request). Notifikasi masuk tidak dibalas.
    pub fn handle(&self, request: &Value) -> Vec<Value> {
        let id = request.get("id").cloned();
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return vec![error_reply(id, INVALID_REQUEST, "pesan tanpa method")];
        };
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            // Notifikasi. `session/cancel` tidak punya efek karena prompt berjalan sinkron.
            return Vec::new();
        };
        match self.dispatch(method, &params) {
            Ok((mut notifications, result)) => {
                notifications.push(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
                notifications
            }
            Err((code, message)) => vec![error_reply(Some(id), code, &message)],
        }
    }

    fn dispatch(&self, method: &str, params: &Value) -> Result<(Vec<Value>, Value), Failure> {
        match method {
            "initialize" => Ok((Vec::new(), initialize_result())),
            "session/new" => self.new_session(params).map(|r| (Vec::new(), r)),
            "session/prompt" => self.prompt(params),
            "_plaesy/document/get" => {
                document(params, |k, i| self.store.get(k, i).map(|d| json!(d)))
            }
            "_plaesy/document/put" => {
                let doc = params
                    .get("document")
                    .cloned()
                    .ok_or((INVALID_PARAMS, "params.document wajib".to_owned()))?;
                document(params, |k, i| {
                    self.store.put(k, i, &doc).map(|()| json!({ "ok": true }))
                })
            }
            "_plaesy/document/list" => {
                let kind = text(params, "kind")?;
                self.store
                    .list(&kind)
                    .map(|ids| (Vec::new(), json!({ "ids": ids })))
                    .map_err(storage)
            }
            "_plaesy/document/delete" => document(params, |k, i| {
                self.store
                    .delete(k, i)
                    .map(|existed| json!({ "deleted": existed }))
            }),
            _ => Err((METHOD_NOT_FOUND, format!("method tidak dikenal: {method}"))),
        }
    }

    fn new_session(&self, params: &Value) -> Result<Value, Failure> {
        let cwd = text(params, "cwd")?;
        if !Path::new(&cwd).is_absolute() {
            return Err((INVALID_PARAMS, "params.cwd harus path absolut".to_owned()));
        }
        if params.get("mcpServers").is_some_and(|v| !v.is_array()) {
            return Err((INVALID_PARAMS, "params.mcpServers harus array".to_owned()));
        }
        let mut sessions = self.sessions.lock().map_err(|_| poisoned())?;
        let session_id = format!("sess-{}", sessions.len() + 1);
        sessions.push(session_id.clone());
        Ok(json!({ "sessionId": session_id }))
    }

    fn prompt(&self, params: &Value) -> Result<(Vec<Value>, Value), Failure> {
        let session_id = text(params, "sessionId")?;
        let known = self
            .sessions
            .lock()
            .map_err(|_| poisoned())?
            .contains(&session_id);
        if !known {
            return Err((
                INVALID_PARAMS,
                format!("sessionId tidak dikenal: {session_id}"),
            ));
        }
        if !params.get("prompt").is_some_and(Value::is_array) {
            return Err((INVALID_PARAMS, "params.prompt harus array".to_owned()));
        }
        let update = json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": session_id,
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": { "type": "text", "text": NO_MODEL_TEXT }
                }
            }
        });
        Ok((vec![update], json!({ "stopReason": "end_turn" })))
    }
}

/// Melayani pesan baris demi baris dari `input` dan menulis balasan ke `output`.
///
/// # Errors
///
/// Galat I/O pada `input` atau `output`.
pub fn serve<R: BufRead, W: Write>(agent: &Agent<'_>, input: R, mut output: W) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let messages = match serde_json::from_str::<Value>(&line) {
            Ok(request) => agent.handle(&request),
            Err(error) => vec![error_reply(
                None,
                PARSE_ERROR,
                &format!("JSON tidak valid: {error}"),
            )],
        };
        for message in messages {
            writeln!(output, "{message}")?;
        }
        output.flush()?;
    }
    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "agentCapabilities": {
            "loadSession": false,
            "promptCapabilities": { "image": false, "audio": false, "embeddedContext": false },
            "mcpCapabilities": { "http": false, "sse": false }
        },
        "agentInfo": { "name": "plaesy", "version": env!("CARGO_PKG_VERSION") },
        "authMethods": []
    })
}

fn error_reply(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message }
    })
}

fn text(params: &Value, key: &str) -> Result<String, Failure> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| (INVALID_PARAMS, format!("params.{key} wajib berupa string")))
}

fn storage(error: StoreError) -> Failure {
    match error {
        StoreError::InvalidKey(message) => (INVALID_PARAMS, message),
        other => (SERVER_ERROR, other.to_string()),
    }
}

fn document(
    params: &Value,
    action: impl FnOnce(&str, &str) -> Result<Value, StoreError>,
) -> Result<(Vec<Value>, Value), Failure> {
    let kind = text(params, "kind")?;
    let id = text(params, "id")?;
    action(&kind, &id)
        .map(|result| (Vec::new(), result))
        .map_err(storage)
}

fn poisoned() -> Failure {
    (
        SERVER_ERROR,
        "status sesi terkunci karena panik sebelumnya".to_owned(),
    )
}

#[cfg(test)]
#[path = "acp_test.rs"]
mod acp_test;
