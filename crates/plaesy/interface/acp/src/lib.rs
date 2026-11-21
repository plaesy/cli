//! Agen ACP v1 lewat stdio (ADR 0003: interface/acp). Spesifikasi:
//! <https://agentclientprotocol.com/protocol/v1/overview>.
//!
//! Agen menerima metode dari klien: `initialize`, `session/new`, `session/load`,
//! `session/prompt`, dan notifikasi `session/cancel`. Agen juga memanggil klien lewat
//! `fs/read_text_file`, `fs/write_text_file`, `terminal/*`, dan `session/request_permission`.
//! Pemanggilan itu hanya dipakai bila klien menyatakan kapabilitasnya saat `initialize`.
//!
//! Sesi disimpan di penyimpanan dokumen (`acp.session`), sehingga `session/load` bisa
//! membaca ulang riwayat setelah proses dimulai ulang.
//!
//! Plaesy belum punya model bahasa terpasang secara default. Model dipasang lewat
//! [`ModelBackend`]. Model menerima [`ToolHost`] untuk memakai kemampuan klien.
//!
//! Operasi dokumen tersedia sebagai ekstensi `_plaesy/document/*`. Aturan penamaan
//! ekstensi ACP belum diverifikasi di spesifikasi.
//!
//! Stdout hanya berisi pesan JSON-RPC, satu per baris.

use plaesy_service::port::{DocumentStore, StoreError};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Versi protokol ACP yang didukung (bilangan bulat di spesifikasi v1).
pub const PROTOCOL_VERSION: u64 = 1;

/// Pesan yang dikirim saat `session/prompt`, bila belum ada model terpasang.
pub const NO_MODEL_TEXT: &str =
    "Plaesy belum punya model bahasa terpasang. Pesan diterima, tetapi tidak ada jawaban yang dihasilkan.";

const SESSION_KIND: &str = "acp.session";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const SERVER_ERROR: i64 = -32000;

type Failure = (i64, String);

/// Pesan masuk dari klien.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// JSON yang valid.
    Message(Value),
    /// Baris yang bukan JSON. Isinya berupa pesan galat parser.
    Invalid(String),
}

/// Saluran dua arah antara agen dan klien.
pub trait Transport {
    /// Mengirim satu pesan.
    ///
    /// # Errors
    ///
    /// Galat I/O.
    fn send(&mut self, message: &Value) -> io::Result<()>;

    /// Menerima pesan berikutnya. `None` berarti klien menutup koneksi.
    ///
    /// # Errors
    ///
    /// Galat I/O.
    fn recv(&mut self) -> io::Result<Option<Incoming>>;
}

/// Transport di atas stdio: satu pesan JSON per baris.
#[derive(Debug)]
pub struct Stdio<R, W> {
    input: R,
    output: W,
}

impl<R: BufRead, W: Write> Stdio<R, W> {
    /// Membungkus `input` dan `output`.
    #[must_use]
    pub fn new(input: R, output: W) -> Self {
        Self { input, output }
    }
}

impl<R: BufRead, W: Write> Transport for Stdio<R, W> {
    fn send(&mut self, message: &Value) -> io::Result<()> {
        writeln!(self.output, "{message}")?;
        self.output.flush()
    }

    fn recv(&mut self) -> io::Result<Option<Incoming>> {
        loop {
            let mut line = String::new();
            if self.input.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            if line.trim().is_empty() {
                continue;
            }
            return Ok(Some(match serde_json::from_str::<Value>(&line) {
                Ok(value) => Incoming::Message(value),
                Err(error) => Incoming::Invalid(error.to_string()),
            }));
        }
    }
}

/// Hasil menjalankan satu perintah lewat terminal klien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub output: String,
    /// `None` bila perintah belum selesai atau dihentikan oleh sinyal.
    pub exit_code: Option<i64>,
}

/// Kemampuan klien yang bisa dipakai model selama satu prompt. Setiap pemanggilan ditolak
/// bila klien tidak menyatakan kapabilitas yang diperlukan.
pub trait ToolHost {
    /// Membaca berkas teks. `path` harus absolut dan berada di bawah cwd sesi.
    ///
    /// # Errors
    ///
    /// Path tidak diizinkan, kapabilitas tidak ada, atau klien gagal.
    fn read_text_file(&mut self, path: &str) -> Result<String, String>;

    /// Menulis berkas teks dengan aturan path yang sama.
    ///
    /// # Errors
    ///
    /// Path tidak diizinkan, kapabilitas tidak ada, atau klien gagal.
    fn write_text_file(&mut self, path: &str, content: &str) -> Result<(), String>;

    /// Meminta izin pengguna. `options` berisi pasangan `(optionId, name)`. Mengembalikan
    /// `optionId` yang dipilih, atau `None` bila dibatalkan.
    ///
    /// # Errors
    ///
    /// Klien gagal menjawab.
    fn request_permission(
        &mut self,
        title: &str,
        options: &[(&str, &str)],
    ) -> Result<Option<String>, String>;

    /// Menjalankan `command` dengan `args` di terminal klien dan menunggu selesai.
    ///
    /// # Errors
    ///
    /// Kapabilitas terminal tidak ada, atau klien gagal.
    fn run_command(&mut self, command: &str, args: &[String]) -> Result<CommandResult, String>;
}

/// Model bahasa yang dipanggil untuk satu pesan pengguna.
pub trait ModelBackend: Send + Sync {
    /// Menghasilkan jawaban untuk `prompt`. `tools` menyediakan kemampuan klien.
    ///
    /// # Errors
    ///
    /// Pesan galat yang bisa ditampilkan ke pengguna.
    fn complete(&self, prompt: &str, tools: &mut dyn ToolHost) -> Result<String, String>;
}

/// Agen ACP di atas sebuah penyimpanan dokumen.
pub struct Agent<'a> {
    store: &'a dyn DocumentStore,
    model: Option<Box<dyn ModelBackend + 'a>>,
    /// `clientCapabilities` dari `initialize`. `Null` sebelum initialize.
    client: Mutex<Value>,
    next_id: AtomicU64,
}

impl<'a> Agent<'a> {
    /// Agen tanpa model. `session/prompt` menjawab dengan pesan bahwa belum ada model.
    #[must_use]
    pub fn new(store: &'a dyn DocumentStore) -> Self {
        Self {
            store,
            model: None,
            client: Mutex::new(Value::Null),
            next_id: AtomicU64::new(0),
        }
    }

    /// Agen dengan model bahasa.
    #[must_use]
    pub fn with_model(store: &'a dyn DocumentStore, model: Box<dyn ModelBackend + 'a>) -> Self {
        Self {
            store,
            model: Some(model),
            client: Mutex::new(Value::Null),
            next_id: AtomicU64::new(0),
        }
    }

    /// Memproses satu pesan masuk dan menulis notifikasi serta balasannya ke `io`.
    /// Notifikasi masuk (tanpa id) tidak dibalas.
    ///
    /// # Errors
    ///
    /// Galat I/O pada transport.
    pub fn handle(&self, request: &Value, io: &mut dyn Transport) -> io::Result<()> {
        let id = request.get("id").cloned();
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return io.send(&error_reply(id, INVALID_REQUEST, "pesan tanpa method"));
        };
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            // Notifikasi. `session/cancel` tidak punya efek karena prompt berjalan sinkron.
            return Ok(());
        };
        let message = match self.dispatch(method, &params, io) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error_reply(Some(id), code, &message),
        };
        io.send(&message)
    }

    fn dispatch(
        &self,
        method: &str,
        params: &Value,
        io: &mut dyn Transport,
    ) -> Result<Value, Failure> {
        match method {
            "initialize" => {
                let caps = params
                    .get("clientCapabilities")
                    .cloned()
                    .unwrap_or(json!({}));
                *self.client.lock().map_err(|_| poisoned())? = caps;
                Ok(initialize_result())
            }
            "session/new" => self.new_session(params),
            "session/load" => self.load_session(params, io),
            "session/prompt" => self.prompt(params, io),
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
                    .map(|ids| json!({ "ids": ids }))
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
        check_mcp_servers(params)?;
        let session_id = format!("sess-{}", uuid::Uuid::new_v4());
        let session = json!({ "cwd": cwd, "messages": [] });
        self.store
            .put(SESSION_KIND, &session_id, &session)
            .map_err(storage)?;
        Ok(json!({ "sessionId": session_id }))
    }

    fn load_session(&self, params: &Value, io: &mut dyn Transport) -> Result<Value, Failure> {
        let session_id = text(params, "sessionId")?;
        let cwd = text(params, "cwd")?;
        check_mcp_servers(params)?;
        let session = self.session(&session_id)?;
        if session["cwd"].as_str() != Some(cwd.as_str()) {
            return Err((INVALID_PARAMS, "cwd tidak cocok dengan sesi".to_owned()));
        }
        // Riwayat dikirim ulang sebagai notifikasi, lalu balasan null (sesuai spesifikasi).
        for message in session["messages"].as_array().into_iter().flatten() {
            let kind = if message["role"].as_str() == Some("user") {
                "user_message_chunk"
            } else {
                "agent_message_chunk"
            };
            io.send(&update(
                &session_id,
                kind,
                message["text"].as_str().unwrap_or(""),
            ))
            .map_err(io_failure)?;
        }
        Ok(Value::Null)
    }

    fn prompt(&self, params: &Value, io: &mut dyn Transport) -> Result<Value, Failure> {
        let session_id = text(params, "sessionId")?;
        let mut session = self.session(&session_id)?;
        if !params.get("prompt").is_some_and(Value::is_array) {
            return Err((INVALID_PARAMS, "params.prompt harus array".to_owned()));
        }
        let prompt = prompt_text(params);
        let reply_text = match &self.model {
            None => NO_MODEL_TEXT.to_owned(),
            Some(model) => {
                let caps = self.client.lock().map_err(|_| poisoned())?.clone();
                let mut tools = SessionTools {
                    io,
                    ids: &self.next_id,
                    session_id: session_id.clone(),
                    cwd: session["cwd"].as_str().unwrap_or("").to_owned(),
                    caps,
                };
                model
                    .complete(&prompt, &mut tools)
                    .map_err(|message| (SERVER_ERROR, format!("model gagal: {message}")))?
            }
        };
        // Riwayat hanya bertambah bila jawaban berhasil, agar riwayat tidak berisi jawaban palsu.
        if let Some(messages) = session["messages"].as_array_mut() {
            messages.push(json!({ "role": "user", "text": prompt }));
            messages.push(json!({ "role": "assistant", "text": reply_text }));
        }
        self.store
            .put(SESSION_KIND, &session_id, &session)
            .map_err(storage)?;
        io.send(&update(&session_id, "agent_message_chunk", &reply_text))
            .map_err(io_failure)?;
        Ok(json!({ "stopReason": "end_turn" }))
    }

    fn session(&self, session_id: &str) -> Result<Value, Failure> {
        match self.store.get(SESSION_KIND, session_id).map_err(storage)? {
            Some(session) => Ok(session),
            None => Err((
                INVALID_PARAMS,
                format!("sessionId tidak dikenal: {session_id}"),
            )),
        }
    }
}

/// Kemampuan klien untuk satu prompt. Menulis permintaan ke klien dan menunggu balasannya
/// dari transport yang sama.
struct SessionTools<'s> {
    io: &'s mut dyn Transport,
    ids: &'s AtomicU64,
    session_id: String,
    cwd: String,
    caps: Value,
}

impl SessionTools<'_> {
    fn capability(&self, path: &[&str]) -> bool {
        let mut value = &self.caps;
        for key in path {
            value = &value[*key];
        }
        value.as_bool().unwrap_or(false)
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = format!("agent-{}", self.ids.fetch_add(1, Ordering::Relaxed) + 1);
        self.io
            .send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .map_err(|e| e.to_string())?;
        loop {
            match self.io.recv().map_err(|e| e.to_string())? {
                None => return Err("klien menutup koneksi".to_owned()),
                Some(Incoming::Invalid(_)) => {}
                Some(Incoming::Message(message)) => {
                    if message.get("method").is_none() && message.get("id") == Some(&json!(id)) {
                        if let Some(error) = message.get("error") {
                            return Err(error["message"]
                                .as_str()
                                .unwrap_or("galat klien")
                                .to_owned());
                        }
                        return Ok(message.get("result").cloned().unwrap_or(Value::Null));
                    }
                    if let (Some(request_id), Some(_)) = (message.get("id"), message.get("method"))
                    {
                        // Klien memanggil agen saat agen menunggu. Agen tidak melayani ini.
                        self.io
                            .send(&error_reply(
                                Some(request_id.clone()),
                                METHOD_NOT_FOUND,
                                "agen sedang menunggu balasan klien",
                            ))
                            .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }
}

impl ToolHost for SessionTools<'_> {
    fn read_text_file(&mut self, path: &str) -> Result<String, String> {
        if !self.capability(&["fs", "readTextFile"]) {
            return Err("klien tidak mendukung fs/read_text_file".to_owned());
        }
        check_path(&self.cwd, path)?;
        let result = self.call(
            "fs/read_text_file",
            json!({ "sessionId": self.session_id, "path": path }),
        )?;
        result["content"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "balasan fs/read_text_file tidak berisi content".to_owned())
    }

    fn write_text_file(&mut self, path: &str, content: &str) -> Result<(), String> {
        if !self.capability(&["fs", "writeTextFile"]) {
            return Err("klien tidak mendukung fs/write_text_file".to_owned());
        }
        check_path(&self.cwd, path)?;
        self.require_permission(&format!("Tulis berkas {path}"))?;
        self.call(
            "fs/write_text_file",
            json!({ "sessionId": self.session_id, "path": path, "content": content }),
        )
        .map(|_| ())
    }

    fn request_permission(
        &mut self,
        title: &str,
        options: &[(&str, &str)],
    ) -> Result<Option<String>, String> {
        self.request_permission_inner(title, options)
    }

    fn run_command(&mut self, command: &str, args: &[String]) -> Result<CommandResult, String> {
        self.run_command_inner(command, args)
    }
}

impl SessionTools<'_> {
    /// Meminta izin pengguna. Tanpa izin, tindakan berdampak ditolak.
    fn require_permission(&mut self, title: &str) -> Result<(), String> {
        match self.request_permission_inner(title, &[("allow", "Izinkan")])? {
            Some(_) => Ok(()),
            None => Err("ditolak pengguna".to_owned()),
        }
    }

    fn request_permission_inner(
        &mut self,
        title: &str,
        options: &[(&str, &str)],
    ) -> Result<Option<String>, String> {
        let option_values: Vec<Value> = options
            .iter()
            .map(|(id, name)| json!({ "optionId": id, "name": name, "kind": "allow_once" }))
            .collect();
        let tool_call_id = format!("plaesy-{}", uuid::Uuid::new_v4());
        let result = self.call(
            "session/request_permission",
            json!({
                "sessionId": self.session_id,
                "toolCall": { "toolCallId": tool_call_id, "title": title },
                "options": option_values
            }),
        )?;
        match result["outcome"]["outcome"].as_str() {
            Some("selected") => Ok(result["outcome"]["optionId"].as_str().map(str::to_owned)),
            Some("cancelled") => Ok(None),
            _ => Err("balasan session/request_permission tidak dikenal".to_owned()),
        }
    }

    /// Menjalankan perintah di terminal klien setelah izin pengguna diterima.
    fn run_command_inner(
        &mut self,
        command: &str,
        args: &[String],
    ) -> Result<CommandResult, String> {
        if !self.capability(&["terminal"]) {
            return Err("klien tidak mendukung terminal".to_owned());
        }
        self.require_permission(&format!("Jalankan perintah {command} {}", args.join(" ")))?;
        let created = self.call(
            "terminal/create",
            json!({ "sessionId": self.session_id, "command": command, "args": args, "cwd": self.cwd }),
        )?;
        let terminal_id = created["terminalId"]
            .as_str()
            .ok_or_else(|| "balasan terminal/create tidak berisi terminalId".to_owned())?
            .to_owned();
        let waited = self.call(
            "terminal/wait_for_exit",
            json!({ "sessionId": self.session_id, "terminalId": terminal_id }),
        );
        let output = self.call(
            "terminal/output",
            json!({ "sessionId": self.session_id, "terminalId": terminal_id }),
        );
        // Terminal dilepas apa pun hasilnya, lalu galat pertama dilaporkan.
        self.call(
            "terminal/release",
            json!({ "sessionId": self.session_id, "terminalId": terminal_id }),
        )?;
        let waited = waited?;
        let output = output?;
        Ok(CommandResult {
            output: output["output"].as_str().unwrap_or("").to_owned(),
            exit_code: waited["exitCode"].as_i64(),
        })
    }
}

/// Memastikan path absolut, tanpa `..`, dan berada di bawah `cwd`.
fn check_path(cwd: &str, path: &str) -> Result<(), String> {
    let candidate = Path::new(path);
    if !candidate.is_absolute() {
        return Err("path harus absolut".to_owned());
    }
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("path tidak boleh memuat ..".to_owned());
    }
    if cwd.is_empty() || !candidate.starts_with(cwd) {
        return Err("path di luar cwd sesi".to_owned());
    }
    // Symlink di dalam cwd bisa menunjuk ke luar. Bandingkan path nyata dari induk yang sudah ada.
    if let Ok(root) = std::fs::canonicalize(cwd) {
        let mut probe = candidate;
        while !probe.exists() {
            match probe.parent() {
                Some(parent) => probe = parent,
                None => break,
            }
        }
        if let Ok(real) = std::fs::canonicalize(probe) {
            if !real.starts_with(&root) {
                return Err("path di luar cwd sesi (lewat symlink)".to_owned());
            }
        }
    }
    Ok(())
}

/// Melayani pesan dari `input` dan menulis balasan ke `output` sampai input habis.
///
/// # Errors
///
/// Galat I/O pada `input` atau `output`.
pub fn serve<R: BufRead, W: Write>(agent: &Agent<'_>, input: R, output: W) -> io::Result<()> {
    let mut io = Stdio::new(input, output);
    while let Some(incoming) = io.recv()? {
        match incoming {
            Incoming::Message(request) => agent.handle(&request, &mut io)?,
            Incoming::Invalid(error) => io.send(&error_reply(
                None,
                PARSE_ERROR,
                &format!("JSON tidak valid: {error}"),
            ))?,
        }
    }
    Ok(())
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "agentCapabilities": {
            "loadSession": true,
            "promptCapabilities": { "image": false, "audio": false, "embeddedContext": false },
            "mcpCapabilities": { "http": false, "sse": false }
        },
        "agentInfo": { "name": "plaesy", "version": env!("CARGO_PKG_VERSION") },
        "authMethods": []
    })
}

fn check_mcp_servers(params: &Value) -> Result<(), Failure> {
    if params.get("mcpServers").is_some_and(|v| !v.is_array()) {
        return Err((INVALID_PARAMS, "params.mcpServers harus array".to_owned()));
    }
    Ok(())
}

fn update(session_id: &str, kind: &str, text: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session_id,
            "update": {
                "sessionUpdate": kind,
                "content": { "type": "text", "text": text }
            }
        }
    })
}

fn error_reply(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "code": code, "message": message }
    })
}

/// Teks dari blok `text` dalam `params.prompt`, dipisah baris baru. Blok lain diabaikan.
fn prompt_text(params: &Value) -> String {
    params
        .get("prompt")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
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

fn io_failure(error: io::Error) -> Failure {
    (SERVER_ERROR, error.to_string())
}

fn document(
    params: &Value,
    action: impl FnOnce(&str, &str) -> Result<Value, StoreError>,
) -> Result<Value, Failure> {
    let kind = text(params, "kind")?;
    let id = text(params, "id")?;
    action(&kind, &id).map_err(storage)
}

fn poisoned() -> Failure {
    (
        SERVER_ERROR,
        "status agen terkunci karena panik sebelumnya".to_owned(),
    )
}

#[cfg(test)]
#[path = "acp_test.rs"]
mod acp_test;
