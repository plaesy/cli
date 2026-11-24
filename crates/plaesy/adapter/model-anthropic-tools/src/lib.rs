//! Model Anthropic Messages API dengan loop `tool_use` untuk ACP (ADR 0003).
//!
//! OPT-IN. Crate ini MELANGGAR aturan offline di CLAUDE.md, karena runtime harus tetap offline dan
//! endpoint Anthropic adalah layanan cloud. Crate ini hanya dibuat supaya tersedia bila owner
//! mengizinkannya. CLI `plaesy acp` tidak menyambungkannya secara default.
//!
//! Bentuk permintaan: `POST {endpoint}/v1/messages` dengan header `x-api-key` (dari variabel
//! lingkungan `api_key_env`) dan `anthropic-version: 2023-06-01`. Alat dikirim sebagai `tools`
//! dengan `input_schema`. Respons berisi `content` berupa blok `text` dan `tool_use` (id, name,
//! input). Hasil alat dikirim balik sebagai pesan `user` berisi blok `tool_result` dengan
//! `tool_use_id`. Loop berhenti saat `stop_reason` bukan `tool_use`.
//!
//! Yang belum diverifikasi langsung terhadap dokumen resmi saat ditulis: nilai header
//! `anthropic-version`, field `max_tokens` yang wajib, dan bentuk `stop_reason`. Ketiganya
//! diambil dari pengetahuan API yang sudah ada dan perlu dicek ulang sebelum dipakai.
//!
//! Kunci API tidak pernah dicetak atau masuk ke pesan galat.

use plaesy_interface_acp::{CommandResult, ModelBackend, ToolHost};
use serde_json::{json, Value};
use std::time::Duration;

/// Batas putaran model-alat dalam satu prompt.
pub const MAX_ROUNDS: usize = 8;

/// Nilai `anthropic-version` yang dikirim. Belum diverifikasi ulang terhadap dokumen resmi.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Batas token keluaran per putaran. Field ini wajib pada Messages API.
pub const MAX_TOKENS: u32 = 1024;

/// Model Anthropic.
#[derive(Debug, Clone)]
pub struct AnthropicToolModel {
    pub endpoint: String,
    pub model: String,
    /// Nama variabel lingkungan yang berisi kunci API.
    pub api_key_env: String,
    pub timeout: Duration,
}

impl ModelBackend for AnthropicToolModel {
    fn complete(&self, prompt: &str, tools: &mut dyn ToolHost) -> Result<String, String> {
        let mut messages = vec![json!({ "role": "user", "content": prompt })];
        for _ in 0..MAX_ROUNDS {
            let reply = self.messages_call(&messages)?;
            let content = reply
                .get("content")
                .and_then(Value::as_array)
                .cloned()
                .ok_or_else(|| "balasan model tidak berisi content".to_owned())?;
            let stop_reason = reply
                .get("stop_reason")
                .and_then(Value::as_str)
                .unwrap_or("");
            if stop_reason != "tool_use" {
                return Ok(content
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n"));
            }
            messages.push(json!({ "role": "assistant", "content": content }));
            let mut results = Vec::new();
            for block in &content {
                if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                    continue;
                }
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
                let output = run_tool(name, &input, tools);
                results
                    .push(json!({ "type": "tool_result", "tool_use_id": id, "content": output }));
            }
            messages.push(json!({ "role": "user", "content": results }));
        }
        Err(format!(
            "model masih memanggil alat setelah {MAX_ROUNDS} putaran"
        ))
    }
}

impl AnthropicToolModel {
    fn messages_call(&self, messages: &[Value]) -> Result<Value, String> {
        let url = format!("{}/v1/messages", self.endpoint.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "messages": messages,
            "tools": tool_definitions(),
        });
        let config = ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(self.timeout))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let key = std::env::var(&self.api_key_env)
            .map_err(|_| format!("variabel lingkungan {} tidak diisi", self.api_key_env))?;
        let mut response = agent
            .post(&url)
            .header("content-type", "application/json")
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("x-api-key", &key)
            .send(body.to_string().as_bytes())
            .map_err(|e| format!("endpoint model tidak terjangkau: {e}"))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("membaca balasan model: {e}"))?;
        if status != 200 {
            return Err(format!("endpoint model membalas status {status}"));
        }
        serde_json::from_str(&text).map_err(|e| format!("balasan model bukan JSON: {e}"))
    }
}

/// Definisi alat dalam format `tools` Anthropic (`input_schema`, bukan `parameters`).
#[must_use]
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "read_file",
            "description": "Baca berkas teks. Path harus absolut dan berada di bawah cwd sesi.",
            "input_schema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        },
        {
            "name": "write_file",
            "description": "Tulis berkas teks. Path harus absolut dan berada di bawah cwd sesi.",
            "input_schema": {
                "type": "object",
                "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                "required": ["path", "content"]
            }
        },
        {
            "name": "run_command",
            "description": "Jalankan perintah di terminal klien dan kembalikan keluarannya.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["command"]
            }
        },
        {
            "name": "request_permission",
            "description": "Minta izin pengguna sebelum tindakan yang berdampak. Mengembalikan allowed atau denied.",
            "input_schema": {
                "type": "object",
                "properties": { "title": { "type": "string" } },
                "required": ["title"]
            }
        }
    ])
}

/// Menjalankan satu alat. Galat dikembalikan sebagai teks agar model bisa menanggapinya.
fn run_tool(name: &str, input: &Value, tools: &mut dyn ToolHost) -> String {
    let text = |key: &str| {
        input
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    match name {
        "read_file" => match tools.read_text_file(&text("path")) {
            Ok(content) => content,
            Err(error) => format!("error: {error}"),
        },
        "write_file" => match tools.write_text_file(&text("path"), &text("content")) {
            Ok(()) => "ok".to_owned(),
            Err(error) => format!("error: {error}"),
        },
        "run_command" => {
            let args: Vec<String> = input
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            match tools.run_command(&text("command"), &args) {
                Ok(CommandResult { output, exit_code }) => format!("exit={exit_code:?}\n{output}"),
                Err(error) => format!("error: {error}"),
            }
        }
        "request_permission" => {
            match tools.request_permission(&text("title"), &[("allow", "Izinkan")]) {
                Ok(Some(_)) => "allowed".to_owned(),
                Ok(None) => "denied".to_owned(),
                Err(error) => format!("error: {error}"),
            }
        }
        other => format!("error: alat {other:?} tidak dikenal"),
    }
}

#[cfg(test)]
#[path = "anthropic_test.rs"]
mod anthropic_test;
