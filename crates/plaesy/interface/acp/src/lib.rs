//! Server JSON-RPC 2.0 lewat stdio untuk agen (ADR 0003: interface/acp).
//!
//! PERINGATAN: nama ACP di ADR tidak merinci protokolnya. Bentuk di sini adalah default
//! sementara: `initialize`, `document/get`, `document/put`, `document/list`, dan
//! `document/delete`. Cocokkan dengan spesifikasi ACP yang disepakan sebelum dipakai
//! pihak lain. Stdout hanya berisi JSON-RPC, satu pesan per baris.

use plaesy_service::port::{DocumentStore, StoreError};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// Versi bentuk protokol sementara ini.
pub const PROTOCOL_VERSION: &str = "0-draft";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const SERVER_ERROR: i64 = -32000;

/// Memproses satu pesan JSON-RPC. `None` untuk notifikasi (tanpa id).
pub fn handle(store: &dyn DocumentStore, request: &Value) -> Option<Value> {
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Some(error_reply(
            request.get("id").cloned(),
            INVALID_REQUEST,
            "pesan tanpa method",
        ));
    };
    let id = request.get("id").cloned()?;
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let outcome = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": { "name": "plaesy", "version": env!("CARGO_PKG_VERSION") }
        })),
        "document/get" => with_key(&params, |kind, id| {
            store.get(kind, id).map(|doc| json!(doc))
        }),
        "document/put" => match params.get("document").cloned() {
            None => Err((INVALID_PARAMS, "params.document wajib".to_owned())),
            Some(document) => with_key(&params, |kind, id| {
                store
                    .put(kind, id, &document)
                    .map(|()| json!({ "ok": true }))
            }),
        },
        "document/list" => text(&params, "kind").and_then(|kind| {
            store
                .list(&kind)
                .map(|ids| json!({ "ids": ids }))
                .map_err(storage)
        }),
        "document/delete" => with_key(&params, |kind, id| {
            store
                .delete(kind, id)
                .map(|existed| json!({ "deleted": existed }))
        }),
        _ => Err((METHOD_NOT_FOUND, format!("method tidak dikenal: {method}"))),
    };
    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error_reply(Some(id), code, &message),
    })
}

/// Membaca pesan baris demi baris dari `input` dan menulis balasan ke `output`.
///
/// # Errors
///
/// Galat I/O pada `input` atau `output`.
pub fn serve<R: BufRead, W: Write>(
    store: &dyn DocumentStore,
    input: R,
    mut output: W,
) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle(store, &request),
            Err(error) => Some(error_reply(
                None,
                PARSE_ERROR,
                &format!("JSON tidak valid: {error}"),
            )),
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

type Outcome = Result<Value, (i64, String)>;

fn error_reply(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message }
    })
}

fn text(params: &Value, key: &str) -> Result<String, (i64, String)> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| (INVALID_PARAMS, format!("params.{key} wajib berupa string")))
}

fn storage(error: StoreError) -> (i64, String) {
    // Kunci tidak valid adalah kesalahan pemanggil; sisanya kesalahan server.
    match error {
        StoreError::InvalidKey(message) => (INVALID_PARAMS, message),
        other => (SERVER_ERROR, other.to_string()),
    }
}

fn with_key(
    params: &Value,
    action: impl FnOnce(&str, &str) -> Result<Value, StoreError>,
) -> Outcome {
    let kind = text(params, "kind")?;
    let id = text(params, "id")?;
    action(&kind, &id).map_err(storage)
}

#[cfg(test)]
#[path = "acp_test.rs"]
mod acp_test;
