//! Model Ollama lokal dengan pemanggilan alat (ADR 0003, ACP). Memanggil `POST {endpoint}/api/chat`
//! dengan `tools`. Setiap `tool_calls` dijalankan lewat [`ToolHost`] klien ACP, hasilnya dikirim
//! balik sebagai pesan `tool`, dan loop berhenti saat model menjawab tanpa alat.
//!
//! Alat yang tersedia: `read_file`, `write_file`, `run_command`, dan `request_permission`.
//! Batas putaran dan batas waktu mencegah loop tak berujung. Tidak ada jaringan selain ke
//! `endpoint` yang diberikan, dan endpoint itu diharapkan lokal.

use plaesy_interface_acp::{CommandResult, ModelBackend, ToolHost};
use serde_json::{json, Value};
use std::time::Duration;

/// Batas putaran model-alat dalam satu prompt.
pub const MAX_ROUNDS: usize = 8;

/// Model Ollama.
#[derive(Debug, Clone)]
pub struct OllamaModel {
    pub endpoint: String,
    pub model: String,
    pub timeout: Duration,
}

impl ModelBackend for OllamaModel {
    fn complete(&self, prompt: &str, tools: &mut dyn ToolHost) -> Result<String, String> {
        let mut messages = vec![json!({ "role": "user", "content": prompt })];
        for _ in 0..MAX_ROUNDS {
            let reply = self.chat(&messages)?;
            let message = reply
                .get("message")
                .cloned()
                .ok_or_else(|| "balasan ollama tidak berisi message".to_owned())?;
            let calls = message
                .get("tool_calls")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if calls.is_empty() {
                return Ok(message
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned());
            }
            messages.push(message);
            for call in &calls {
                let name = call["function"]["name"].as_str().unwrap_or("");
                let arguments = call["function"]["arguments"].clone();
                let result = run_tool(name, &arguments, tools);
                messages.push(json!({ "role": "tool", "tool_name": name, "content": result }));
            }
        }
        Err(format!(
            "model masih memanggil alat setelah {MAX_ROUNDS} putaran"
        ))
    }
}

impl OllamaModel {
    fn chat(&self, messages: &[Value]) -> Result<Value, String> {
        let url = format!("{}/api/chat", self.endpoint.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "messages": messages,
            "tools": tool_definitions(),
            "stream": false
        });
        let config = ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(self.timeout))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let mut response = agent
            .post(&url)
            .header("content-type", "application/json")
            .send(body.to_string().as_bytes())
            .map_err(|e| format!("ollama tidak terjangkau: {e}"))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("membaca balasan ollama: {e}"))?;
        if status != 200 {
            return Err(format!("ollama membalas status {status}"));
        }
        serde_json::from_str(&text).map_err(|e| format!("balasan ollama bukan JSON: {e}"))
    }
}

/// Definisi alat dalam format `tools` Ollama.
#[must_use]
pub fn tool_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Baca berkas teks. Path harus absolut dan berada di bawah cwd sesi.",
                "parameters": {
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Tulis berkas teks. Path harus absolut dan berada di bawah cwd sesi.",
                "parameters": {
                    "type": "object",
                    "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Jalankan perintah di terminal klien dan kembalikan keluarannya.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "args": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "request_permission",
                "description": "Minta izin pengguna sebelum tindakan yang berdampak. Mengembalikan allowed atau denied.",
                "parameters": {
                    "type": "object",
                    "properties": { "title": { "type": "string" } },
                    "required": ["title"]
                }
            }
        }
    ])
}

/// Menjalankan satu alat dan mengembalikan teks hasilnya untuk model. Galat juga dikembalikan
/// sebagai teks agar model bisa menanggapinya, bukan menghentikan prompt.
fn run_tool(name: &str, arguments: &Value, tools: &mut dyn ToolHost) -> String {
    let text = |key: &str| {
        arguments
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
            let args: Vec<String> = arguments
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            match tools.run_command(&text("command"), &args) {
                Ok(CommandResult { output, exit_code }) => {
                    format!("exit={exit_code:?}\n{output}")
                }
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
#[path = "ollama_test.rs"]
mod ollama_test;
