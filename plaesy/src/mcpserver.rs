//! A stdio MCP server that exposes a project's `.plaesy/` folder.
//!
//! The server lists and reads files under `.plaesy/`, and writes text files
//! there. It never deletes, and it never writes outside `.plaesy/`. Returned
//! content is project data, not instructions.
//! Port of `scripts/internal/mcpserver` (Go).

use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// MCP revisions this server speaks, newest first. Only the tools capability is
/// used, and it is the same in each revision.
const SUPPORTED_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const SERVER_NAME: &str = "plaesy";
const MAX_READ_BYTES: usize = 512 * 1024;
const MAX_WRITE_BYTES: usize = 512 * 1024;
const MAX_LIST_ENTRIES: usize = 2000;

/// Text formats the read and write tools accept.
const READABLE_EXTENSIONS: [&str; 7] = ["md", "json", "yaml", "yml", "txt", "csv", "toml"];

/// JSON-RPC error with its code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }
}

/// Serves one project's `.plaesy/` folder.
#[derive(Debug, Clone)]
pub struct Server {
    base: PathBuf,
    version: String,
}

impl Server {
    /// Serves `<project_root>/.plaesy`.
    ///
    /// # Errors
    ///
    /// The project root cannot be made absolute.
    pub fn new(project_root: &Path, version: &str) -> io::Result<Self> {
        let root = if project_root.is_absolute() {
            project_root.to_path_buf()
        } else {
            std::env::current_dir()?.join(project_root)
        };
        Ok(Self {
            base: root.join(".plaesy"),
            version: version.to_owned(),
        })
    }

    /// Reads newline-delimited JSON-RPC from `input` and writes replies to
    /// `output` until `input` closes. Notifications get no reply.
    ///
    /// # Errors
    ///
    /// I/O failures on either stream.
    pub fn serve(&self, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
        for line in input.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(request) = serde_json::from_str::<Value>(line) else {
                let reply = error_reply(
                    Value::Null,
                    &RpcError {
                        code: -32700,
                        message: "parse error".into(),
                    },
                );
                writeln!(output, "{reply}")?;
                continue;
            };
            if let Some(reply) = self.handle(&request) {
                writeln!(output, "{reply}")?;
            }
        }
        Ok(())
    }

    /// Handles one request. Returns `None` for notifications, which have no `id`.
    #[must_use]
    pub fn handle(&self, request: &Value) -> Option<Value> {
        let id = request.get("id")?.clone();
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        Some(match self.dispatch(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(err) => error_reply(id, &err),
        })
    }

    fn dispatch(&self, method: &str, params: &Value) -> Result<Value, RpcError> {
        match method {
            "initialize" => {
                let requested = params.get("protocolVersion").and_then(Value::as_str);
                let version = requested
                    .filter(|v| SUPPORTED_VERSIONS.contains(v))
                    .unwrap_or(SUPPORTED_VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": self.version },
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => self.call_tool(params),
            other => Err(RpcError {
                code: -32601,
                message: format!("method not found: {other}"),
            }),
        }
    }

    /// A malformed call is a protocol error. A well-formed call that fails while
    /// running is a result with `isError` set.
    fn call_tool(&self, params: &Value) -> Result<Value, RpcError> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::invalid_params("invalid params: name is required"))?;
        let args = params.get("arguments").cloned().unwrap_or(Value::Null);
        let path = args.get("path").and_then(Value::as_str);

        let outcome = match name {
            "plaesy_list" => self.list(path.unwrap_or("")),
            "plaesy_read" => {
                let path = path.ok_or_else(|| RpcError::invalid_params("path is required"))?;
                self.read(path)
            }
            "plaesy_write" => {
                let path =
                    path.ok_or_else(|| RpcError::invalid_params("path and content are required"))?;
                let content = args
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("path and content are required"))?;
                self.write(path, content)
            }
            other => return Err(RpcError::invalid_params(format!("unknown tool: {other}"))),
        };
        Ok(match outcome {
            Ok(text) => tool_result(&text, false),
            Err(message) => tool_result(&message, true),
        })
    }

    /// Lists files under a folder of `.plaesy/`, as `path<TAB>bytes` lines.
    ///
    /// # Errors
    ///
    /// Path refused, folder missing, or not a folder.
    pub fn list(&self, rel: &str) -> Result<String, String> {
        let clean = clean_rel(rel)?;
        let root = self.open_root()?;
        let folder = self.resolve(&root, &clean)?;
        if !folder.is_dir() {
            return Err(format!("not a folder: {rel}"));
        }
        let mut files = Vec::new();
        collect_files(&folder, &mut files);
        let lines: Vec<String> = files
            .into_iter()
            .take(MAX_LIST_ENTRIES)
            .filter_map(|file| {
                let size = fs::metadata(&file).ok()?.len();
                let shown = file
                    .strip_prefix(&root)
                    .ok()?
                    .to_string_lossy()
                    .replace('\\', "/");
                Some(format!("{shown}\t{size}"))
            })
            .collect();
        if lines.is_empty() {
            Ok("(empty)".into())
        } else {
            Ok(lines.join("\n"))
        }
    }

    /// Reads one text file under `.plaesy/`. Content past 512 KB is cut with a note.
    ///
    /// # Errors
    ///
    /// Unsupported type, path refused, or the file cannot be read.
    pub fn read(&self, rel: &str) -> Result<String, String> {
        if rel.trim().is_empty() {
            return Err("path is required".into());
        }
        check_extension(rel)?;
        let clean = clean_rel(rel)?;
        let root = self.open_root()?;
        let file = self.resolve(&root, &clean)?;
        if file.is_dir() {
            return Err(format!("is a folder, not a file: {rel}"));
        }
        let handle = fs::File::open(&file).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        handle
            .take(MAX_READ_BYTES as u64 + 1)
            .read_to_end(&mut buf)
            .map_err(|e| e.to_string())?;
        if buf.len() > MAX_READ_BYTES {
            buf.truncate(MAX_READ_BYTES);
            return Ok(format!(
                "{}\n\n[truncated at {MAX_READ_BYTES} bytes]",
                String::from_utf8_lossy(&buf)
            ));
        }
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Creates or replaces one text file under `.plaesy/`. The write goes to a
    /// temporary file that is renamed into place, so readers never see a partial file.
    ///
    /// # Errors
    ///
    /// Unsupported type, oversize content, the generated `analysis/` folder, a
    /// symlink target, path refused, or I/O failures.
    pub fn write(&self, rel: &str, content: &str) -> Result<String, String> {
        if rel.trim().is_empty() {
            return Err("path is required".into());
        }
        check_extension(rel)?;
        if content.len() > MAX_WRITE_BYTES {
            return Err(format!(
                "content is {} bytes, limit is {MAX_WRITE_BYTES}",
                content.len()
            ));
        }
        let clean = clean_rel(rel)?;
        if clean
            .components()
            .next()
            .is_some_and(|c| c.as_os_str() == "analysis")
        {
            return Err("analysis/ is generated by plaesy analyze and cannot be written".into());
        }
        let root = self.open_root()?;
        let target = self.resolve(&root, &clean)?;
        if target.is_dir() {
            return Err(format!("is a folder, not a file: {rel}"));
        }
        if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!("refusing to write through a symlink: {rel}"));
        }
        let parent = target
            .parent()
            .ok_or_else(|| format!("invalid path: {rel}"))?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let tmp = parent.join(format!(".plaesy-write-{stamp}.tmp"));
        let written = fs::write(&tmp, content).and_then(|()| fs::rename(&tmp, &target));
        if let Err(e) = written {
            let _ = fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        Ok(format!(
            "wrote {} ({} bytes)",
            clean.display(),
            content.len()
        ))
    }

    fn open_root(&self) -> Result<PathBuf, String> {
        if !self.base.is_dir() {
            return Err(".plaesy/ folder not found in the project root".into());
        }
        self.base
            .canonicalize()
            .map_err(|_| ".plaesy/ folder not found in the project root".into())
    }

    /// Joins a clean relative path onto `root`, and refuses it when the nearest
    /// existing ancestor resolves outside `root`, which catches symlinks and junctions.
    fn resolve(&self, root: &Path, clean: &Path) -> Result<PathBuf, String> {
        let full = root.join(clean);
        let mut probe = full.as_path();
        loop {
            if probe.exists() {
                let real = probe.canonicalize().map_err(|e| e.to_string())?;
                if !real.starts_with(root) {
                    return Err("path is outside .plaesy/".into());
                }
                return Ok(full);
            }
            match probe.parent() {
                Some(parent) => probe = parent,
                None => return Err("path is outside .plaesy/".into()),
            }
        }
    }
}

/// Converts a client path into a clean relative path. Absolute paths and any `..`
/// are refused. The empty path is the root.
fn clean_rel(rel: &str) -> Result<PathBuf, String> {
    let mut clean = PathBuf::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return Err("path is outside .plaesy/".into()),
        }
    }
    Ok(clean)
}

fn check_extension(rel: &str) -> Result<(), String> {
    let ext = Path::new(rel)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if READABLE_EXTENSIONS.contains(&ext.as_str()) {
        Ok(())
    } else {
        Err(format!("unsupported file type: {rel}"))
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if out.len() > MAX_LIST_ENTRIES {
            return;
        }
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn tool_result(text: &str, is_error: bool) -> Value {
    let mut result = json!({ "content": [{ "type": "text", "text": text }] });
    if is_error {
        result["isError"] = json!(true);
    }
    result
}

fn error_reply(id: Value, err: &RpcError) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": err.code, "message": err.message } })
}

fn read_only(title: &str) -> Value {
    json!({
        "title": title,
        "readOnlyHint": true,
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": false,
    })
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "plaesy_list",
            "annotations": read_only("List .plaesy files"),
            "description": "List the files in this project's .plaesy/ folder (read-only). Returns relative paths and sizes in bytes.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder relative to .plaesy/. Empty means the root." }
                }
            }
        },
        {
            "name": "plaesy_read",
            "annotations": read_only("Read a .plaesy file"),
            "description": "Read one text file from this project's .plaesy/ folder (read-only). Supported types: md, json, yaml, yml, txt, csv, toml.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File relative to .plaesy/, for example memory.md." }
                },
                "required": ["path"]
            }
        },
        {
            "name": "plaesy_write",
            "annotations": {
                "title": "Write a .plaesy file",
                "readOnlyHint": false,
                "destructiveHint": true,
                "idempotentHint": true,
                "openWorldHint": false,
            },
            "description": "Create or overwrite one text file under this project's .plaesy/ folder. Writes are atomic and limited to md, json, yaml, yml, txt, csv and toml files up to 512 KB. The analysis/ folder is refused because plaesy analyze generates it. Nothing is deleted.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File relative to .plaesy/, for example memory/overview.md. Missing folders are created." },
                    "content": { "type": "string", "description": "Full new content of the file (replaces the existing content)." }
                },
                "required": ["path", "content"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_with_memory() -> (tempfile::TempDir, Server) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy/memory")).unwrap();
        fs::write(dir.path().join(".plaesy/memory/notes.md"), "hello").unwrap();
        let server = Server::new(dir.path(), "0.0.1").unwrap();
        (dir, server)
    }

    #[test]
    fn initialize_echoes_a_supported_version_and_falls_back_otherwise() {
        let (_dir, server) = project_with_memory();
        let reply = server
            .handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}))
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2024-11-05");
        let reply = server
            .handle(&json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}))
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], SUPPORTED_VERSIONS[0]);
    }

    #[test]
    fn notifications_get_no_reply() {
        let (_dir, server) = project_with_memory();
        assert!(server
            .handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .is_none());
    }

    #[test]
    fn list_and_read_stay_inside_plaesy() {
        let (_dir, server) = project_with_memory();
        assert!(server.list("").unwrap().contains("memory/notes.md\t5"));
        assert_eq!(server.read("memory/notes.md").unwrap(), "hello");
        assert!(server.read("../secret.md").is_err());
        assert!(server.read("/etc/passwd").is_err());
        assert!(server.read("image.png").is_err());
    }

    #[test]
    fn write_creates_folders_and_refuses_analysis() {
        let (dir, server) = project_with_memory();
        server
            .write("memory/new/overview.md", "# Overview")
            .unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join(".plaesy/memory/new/overview.md")).unwrap(),
            "# Overview"
        );
        assert!(server.write("analysis/x.json", "{}").is_err());
        assert!(server.write("../outside.md", "x").is_err());
    }

    #[test]
    fn tool_failure_is_a_result_not_a_protocol_error() {
        let (_dir, server) = project_with_memory();
        let reply = server
            .handle(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"plaesy_read","arguments":{"path":"missing.md"}}}))
            .unwrap();
        assert_eq!(reply["result"]["isError"], true);
        let bad = server
            .handle(&json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope","arguments":{}}}))
            .unwrap();
        assert_eq!(bad["error"]["code"], -32602);
    }
}
