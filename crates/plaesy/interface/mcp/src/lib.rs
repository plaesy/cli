//! Server MCP untuk dokumen (ADR 0003: interface/mcp). Berbicara JSON-RPC 2.0 satu pesan
//! per baris. Stdout hanya berisi protokol; diagnostik tidak ditulis ke sini.
//!
//! Metode: `initialize`, `notifications/initialized` (tanpa balasan), `tools/list`, dan
//! `tools/call` untuk `document_get`, `document_put`, `document_list`, `document_delete`.

use plaesy_service::port::{DocumentStore, StoreError};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// Versi protokol MCP yang diumumkan saat `initialize`.
pub const PROTOCOL_VERSION: &str = "2024-11-05";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Memproses satu pesan JSON-RPC. `None` berarti tidak ada balasan (notifikasi).
pub fn handle(store: &dyn DocumentStore, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Some(error_reply(id, INVALID_REQUEST, "pesan tanpa method"));
    };
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    // Notifikasi (tanpa id) tidak dibalas.
    let id = id?;
    let outcome = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "plaesy", "version": env!("CARGO_PKG_VERSION") }
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => call_tool(store, &params),
        _ => Err((METHOD_NOT_FOUND, format!("method tidak dikenal: {method}"))),
    };
    Some(match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message }
        }),
    })
}

/// Membaca pesan dari `input` baris demi baris dan menulis balasan ke `output`.
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

fn error_reply(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message }
    })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "document_get",
            "description": "Ambil satu dokumen JSON berdasarkan kind dan id.",
            "inputSchema": {
                "type": "object",
                "properties": { "kind": { "type": "string" }, "id": { "type": "string" } },
                "required": ["kind", "id"]
            }
        },
        {
            "name": "document_put",
            "description": "Simpan atau timpa dokumen JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string" },
                    "id": { "type": "string" },
                    "document": {}
                },
                "required": ["kind", "id", "document"]
            }
        },
        {
            "name": "document_list",
            "description": "Daftar id dokumen untuk satu kind.",
            "inputSchema": {
                "type": "object",
                "properties": { "kind": { "type": "string" } },
                "required": ["kind"]
            }
        },
        {
            "name": "document_delete",
            "description": "Hapus satu dokumen. Hasilnya menyatakan apakah dokumen ada.",
            "inputSchema": {
                "type": "object",
                "properties": { "kind": { "type": "string" }, "id": { "type": "string" } },
                "required": ["kind", "id"]
            }
        }
    ])
}

type ToolError = (i64, String);

fn call_tool(store: &dyn DocumentStore, params: &Value) -> Result<Value, ToolError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (INVALID_PARAMS, "params.name wajib".to_owned()))?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let text_arg = |key: &str| -> Result<String, ToolError> {
        arguments
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| (INVALID_PARAMS, format!("argumen {key} wajib berupa string")))
    };
    let result: Result<Value, StoreError> = match name {
        "document_get" => store
            .get(&text_arg("kind")?, &text_arg("id")?)
            .map(|doc| json!(doc)),
        "document_put" => {
            let document = arguments
                .get("document")
                .cloned()
                .ok_or_else(|| (INVALID_PARAMS, "argumen document wajib".to_owned()))?;
            store
                .put(&text_arg("kind")?, &text_arg("id")?, &document)
                .map(|()| json!({ "ok": true }))
        }
        "document_list" => store
            .list(&text_arg("kind")?)
            .map(|ids| json!({ "ids": ids })),
        "document_delete" => store
            .delete(&text_arg("kind")?, &text_arg("id")?)
            .map(|existed| json!({ "deleted": existed })),
        other => return Err((INVALID_PARAMS, format!("tool tidak dikenal: {other}"))),
    };
    // Galat penyimpanan dilaporkan sebagai hasil tool dengan isError, sesuai MCP, bukan galat JSON-RPC.
    let (payload, is_error) = match result {
        Ok(value) => (value, false),
        Err(error) => (json!({ "error": error.to_string() }), true),
    };
    Ok(json!({
        "content": [{ "type": "text", "text": payload.to_string() }],
        "isError": is_error
    }))
}

#[cfg(test)]
#[path = "mcp_test.rs"]
mod mcp_test;
