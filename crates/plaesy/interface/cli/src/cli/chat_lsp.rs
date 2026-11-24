//! Read-only code intelligence for `plaesy chat`: `/lsp definition|references|hover <file>:<line>:<col>`.
//! A language server named in `lsp.servers` of `settings.json` answers. Servers talk JSON-RPC with
//! `Content-Length` framing over stdio, the same way editors do. Each `/lsp` call starts the server,
//! asks one question, and stops it, so a hung server cannot block the chat. A repository server runs
//! only after its fingerprint is listed in `approved-hooks.json`, like MCP servers.
//!
//! Lines are 1-based and columns count characters in the file. Language servers count UTF-16 units,
//! so [`utf16_column`] converts before each request. Results are capped at [`MAX_RESULTS`].

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::chat_hooks::Scope;

pub const MAX_RESULTS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub name: String,
    /// File extensions this server answers for, lowercase and without the dot.
    pub extensions: Vec<String>,
    pub command: String,
    pub args: Vec<String>,
    pub timeout: Duration,
    pub scope: Scope,
}

#[derive(Debug, Deserialize)]
struct RawServer {
    name: String,
    extensions: Vec<String>,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout: String,
}

fn default_timeout() -> String {
    "20s".to_owned()
}

/// Servers from a settings value (`"lsp": {"servers": [...]}`).
pub fn parse_servers(value: &Value, scope: Scope) -> Result<Vec<Server>, String> {
    let Some(list) = value.get("lsp").and_then(|lsp| lsp.get("servers")) else {
        return Ok(Vec::new());
    };
    let raws: Vec<RawServer> =
        serde_json::from_value(list.clone()).map_err(|err| format!("lsp.servers: {err}"))?;
    raws.into_iter()
        .map(|raw| {
            if raw.name.trim().is_empty() || raw.command.trim().is_empty() {
                return Err("lsp server needs a name and a command".to_owned());
            }
            let timeout = crate::reach::duration::parse(&raw.timeout)
                .ok_or_else(|| format!("invalid lsp timeout {:?}", raw.timeout))?;
            Ok(Server {
                name: raw.name,
                extensions: raw
                    .extensions
                    .iter()
                    .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
                    .collect(),
                command: raw.command,
                args: raw.args,
                timeout,
                scope,
            })
        })
        .collect()
}

/// A stable id for a server's command line, used for approval of repository servers.
pub fn fingerprint(server: &Server) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"lsp\0");
    hasher.update(server.name.as_bytes());
    hasher.update([0]);
    hasher.update(server.command.as_bytes());
    for arg in &server.args {
        hasher.update([0]);
        hasher.update(arg.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn is_trusted(server: &Server, approved: &[String]) -> bool {
    server.scope == Scope::User || approved.iter().any(|fp| *fp == fingerprint(server))
}

/// The first server that lists the file's extension.
pub fn server_for<'a>(servers: &'a [Server], path: &Path) -> Option<&'a Server> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    servers
        .iter()
        .find(|server| server.extensions.contains(&ext))
}

/// The language id an editor would send for a file.
pub fn language_id(path: &Path) -> &'static str {
    match path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js") => "javascript",
        Some("go") => "go",
        Some("c") => "c",
        Some("cc" | "cpp" | "h" | "hpp") => "cpp",
        _ => "plaintext",
    }
}

/// `Content-Length` framing: a header block, a blank line, then the JSON body.
pub fn encode(message: &Value) -> Vec<u8> {
    let body = message.to_string();
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

pub fn read_message<R: BufRead>(reader: &mut R) -> Result<Value, String> {
    let mut length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .map_err(|err| format!("lsp read: {err}"))?;
        if read == 0 {
            return Err("lsp server closed the connection".to_owned());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length:") {
            length = value.trim().parse().ok();
        }
    }
    let length = length.ok_or_else(|| "lsp message has no Content-Length".to_owned())?;
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|err| format!("lsp read: {err}"))?;
    serde_json::from_slice(&body).map_err(|err| format!("lsp message: {err}"))
}

/// The JSON-RPC conversation over any reader and writer, so tests can use in-memory buffers.
pub struct Client<R: BufRead, W: Write> {
    reader: R,
    writer: W,
    next_id: u64,
}

impl<R: BufRead, W: Write> Client<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            next_id: 1,
        }
    }

    fn send(&mut self, message: &Value) -> Result<(), String> {
        self.writer
            .write_all(&encode(message))
            .and_then(|()| self.writer.flush())
            .map_err(|err| format!("lsp write: {err}"))
    }

    /// Sends a request and returns its `result`. Notifications and replies to other ids are skipped.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        loop {
            let message = read_message(&mut self.reader)?;
            if message.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(format!("{method}: {error}"));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    pub fn initialize(&mut self, root_uri: &str) -> Result<(), String> {
        self.request(
            "initialize",
            json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
        )?;
        self.send(&json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}))
    }

    pub fn open(&mut self, uri: &str, language: &str, text: &str) -> Result<(), String> {
        self.send(&json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri, "languageId": language, "version": 1, "text": text}}
        }))
    }

    pub fn request_at(
        &mut self,
        method: &str,
        uri: &str,
        line: u32,
        character: u32,
    ) -> Result<Value, String> {
        let mut params = json!({
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": character}
        });
        if method == "textDocument/references" {
            params["context"] = json!({"includeDeclaration": true});
        }
        self.request(method, params)
    }
}

/// Starts the server in the repository, runs `work`, and stops the server. A server that does not
/// answer within its timeout is killed.
pub fn with_server<F>(server: &Server, repo: &Path, work: F) -> Result<String, String>
where
    F: FnOnce(&mut Client<BufReader<ChildStdout>, ChildStdin>) -> Result<String, String>
        + Send
        + 'static,
{
    let mut child = Command::new(&server.command)
        .args(&server.args)
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("lsp {} ({}): {err}", server.name, server.command))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("lsp {}: no stdin", server.name))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("lsp {}: no stdout", server.name))?;
    let root_uri = file_uri(repo);
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut client = Client::new(BufReader::new(stdout), stdin);
        let result = client
            .initialize(&root_uri)
            .and_then(|()| work(&mut client));
        let _ = sender.send(result);
    });
    let outcome = receiver.recv_timeout(server.timeout);
    let _ = child.kill();
    let _ = child.wait();
    match outcome {
        Ok(result) => result,
        Err(_) => Err(format!(
            "lsp {} did not answer within {:?}",
            server.name, server.timeout
        )),
    }
}

/// The absolute, normalized form of a path. Servers reject relative `file://` URIs, and Windows
/// verbatim prefixes (`\\?\`) are removed so the URI and the repository path match.
pub fn absolute(path: &Path) -> Result<PathBuf, String> {
    let full = fs::canonicalize(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let text = full.to_string_lossy();
    Ok(PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)))
}

/// `file:///C:/repo/src/a.rs` for a path, with spaces encoded.
pub fn file_uri(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    format!("file:///{}", text.trim_start_matches('/')).replace(' ', "%20")
}

/// The path a `file://` URI names, with forward slashes. Unknown schemes are returned unchanged.
pub fn uri_to_path(uri: &str) -> String {
    match uri.strip_prefix("file:///") {
        Some(rest) => rest
            .replace("%20", " ")
            .replace("%3A", ":")
            .replace("%3a", ":"),
        None => uri.to_owned(),
    }
}

/// UTF-16 offset of a 1-based character column. Language servers count UTF-16 units.
pub fn utf16_column(line: &str, column: usize) -> u32 {
    let units: usize = line
        .chars()
        .take(column.saturating_sub(1))
        .map(char::len_utf16)
        .sum();
    u32::try_from(units).unwrap_or(u32::MAX)
}

/// `path:line:col` with a repository-relative path. The path must stay inside the repository.
pub fn parse_target(target: &str) -> Result<(String, usize, usize), String> {
    let usage = || "usage: /lsp <definition|references|hover> <file>:<line>:<col>".to_owned();
    let mut parts = target.rsplitn(3, ':');
    let (Some(col), Some(line), Some(file)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(usage());
    };
    let line: usize = line.parse().map_err(|_| usage())?;
    let col: usize = col.parse().map_err(|_| usage())?;
    if line == 0 || col == 0 || !relative_inside(file) {
        return Err(usage());
    }
    Ok((file.to_owned(), line, col))
}

fn relative_inside(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && !p.is_absolute()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Turns a definition, references, or hover result into short text, relative to the repository.
pub fn format_result(action: &str, result: &Value, repo: &Path) -> String {
    // Servers may lowercase the drive letter, so the prefix is compared without regard to case.
    let repo_prefix = format!("{}/", file_uri(repo).trim_start_matches("file:///"));
    if action == "hover" {
        let text = hover_text(result);
        return if text.is_empty() {
            "no hover information".to_owned()
        } else {
            text
        };
    }
    let items: Vec<&Value> = match result {
        Value::Array(list) => list.iter().collect(),
        Value::Null => Vec::new(),
        other => vec![other],
    };
    let mut lines = Vec::new();
    for item in items.iter().take(MAX_RESULTS) {
        let uri = item
            .get("uri")
            .or_else(|| item.get("targetUri"))
            .and_then(Value::as_str);
        let range = item
            .get("range")
            .or_else(|| item.get("targetSelectionRange"))
            .or_else(|| item.get("targetRange"));
        let (Some(uri), Some(start)) = (uri, range.and_then(|r| r.get("start"))) else {
            continue;
        };
        let path = uri_to_path(uri);
        let shown = match path.get(..repo_prefix.len()) {
            Some(head) if head.eq_ignore_ascii_case(&repo_prefix) => &path[repo_prefix.len()..],
            _ => path.as_str(),
        };
        let line = start.get("line").and_then(Value::as_u64).unwrap_or(0) + 1;
        let column = start.get("character").and_then(Value::as_u64).unwrap_or(0) + 1;
        lines.push(format!("{shown}:{line}:{column}"));
    }
    if lines.is_empty() {
        return "no results".to_owned();
    }
    if items.len() > MAX_RESULTS {
        lines.push(format!("... and {} more", items.len() - MAX_RESULTS));
    }
    lines.join("\n")
}

fn hover_text(result: &Value) -> String {
    fn piece(value: &Value) -> Option<String> {
        match value {
            Value::String(text) => Some(text.clone()),
            Value::Object(_) => value
                .get("value")
                .and_then(Value::as_str)
                .map(str::to_owned),
            _ => None,
        }
    }
    let Some(contents) = result.get("contents") else {
        return String::new();
    };
    match contents {
        Value::Array(list) => list.iter().filter_map(piece).collect::<Vec<_>>().join("\n"),
        other => piece(other).unwrap_or_default(),
    }
}

/// True when asking again may help: the server is still indexing (`content modified`), or it
/// answered with nothing yet.
pub fn retry_later(answer: &Result<Value, String>) -> bool {
    match answer {
        Err(err) => err.contains("content modified"),
        Ok(value) => is_empty_answer(value),
    }
}

/// True for an answer with nothing in it: null, an empty list, or a hover with no contents.
pub fn is_empty_answer(result: &Value) -> bool {
    match result {
        Value::Null => true,
        Value::Array(list) => list.is_empty(),
        Value::Object(map) => map.get("contents").is_none() && map.get("uri").is_none(),
        _ => false,
    }
}

/// Runs `/lsp <action> <file>:<line>:<col>`. Refused unless the file's server is trusted and the
/// file is inside the repository.
pub fn run(
    servers: &[Server],
    approved: &[String],
    repo: &Path,
    action: &str,
    target: &str,
) -> Result<String, String> {
    let method = match action {
        "definition" => "textDocument/definition",
        "references" => "textDocument/references",
        "hover" => "textDocument/hover",
        _ => {
            return Err("usage: /lsp <definition|references|hover> <file>:<line>:<col>".to_owned())
        }
    };
    let (file, line, column) = parse_target(target)?;
    let repo = absolute(repo)?;
    let path = repo.join(&file);
    let text = fs::read_to_string(&path).map_err(|err| format!("{file}: {err}"))?;
    let server = server_for(servers, &path)
        .ok_or_else(|| format!("no lsp server for {file}; add lsp.servers to settings.json"))?;
    if !is_trusted(server, approved) {
        return Err(format!("lsp server {} is not approved", server.name));
    }
    let source_line = text.lines().nth(line - 1).unwrap_or("");
    let character = utf16_column(source_line, column);
    let uri = file_uri(&path);
    let language = language_id(&path);
    let line0 = u32::try_from(line - 1).unwrap_or(u32::MAX);
    let repo_owned = repo.to_path_buf();
    let action_owned = action.to_owned();
    let budget = server.timeout;
    with_server(server, &repo, move |client| {
        client.open(&uri, language, &text)?;
        // A server that is still indexing answers with an empty result, so ask again for a while.
        // The overall timeout of the server still bounds the wait.
        let started = std::time::Instant::now();
        let mut result = client.request_at(method, &uri, line0, character);
        while started.elapsed() + Duration::from_secs(2) < budget && retry_later(&result) {
            thread::sleep(Duration::from_millis(500));
            result = client.request_at(method, &uri, line0, character);
        }
        let result = result?;
        Ok(format_result(&action_owned, &result, &repo_owned))
    })
}

#[cfg(test)]
#[path = "chat_lsp_test.rs"]
mod chat_lsp_test;
