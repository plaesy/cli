//! MCP client for `plaesy chat`. Servers are configured under `mcp.servers` in `settings.json`
//! and speak newline-delimited JSON-RPC over stdio. Each `/mcp` command starts a server, talks to
//! it, and stops it, so a server that hangs or crashes cannot take the chat session down.
//!
//! Only tools named in `allow_tools` are listed or callable. A repository server runs only after
//! its fingerprint is listed in `approved-hooks.json` (same file as hooks), because it runs a
//! command from the repository.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::chat_hooks::Scope;

pub const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub allow_tools: Vec<String>,
    pub timeout: Duration,
    pub scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct RawServer {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    allow_tools: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout: String,
}

fn default_enabled() -> bool {
    true
}

fn default_timeout() -> String {
    "10s".to_owned()
}

/// Servers from a settings value (`"mcp": {"servers": [...]}`).
pub fn parse_servers(value: &Value, scope: Scope) -> Result<Vec<McpServer>, String> {
    let Some(list) = value.get("mcp").and_then(|mcp| mcp.get("servers")) else {
        return Ok(Vec::new());
    };
    let raws: Vec<RawServer> =
        serde_json::from_value(list.clone()).map_err(|err| format!("mcp.servers: {err}"))?;
    raws.into_iter()
        .map(|raw| {
            if raw.name.trim().is_empty() {
                return Err("mcp server name must not be empty".to_owned());
            }
            let timeout = crate::reach::duration::parse(&raw.timeout)
                .ok_or_else(|| format!("invalid mcp timeout {:?}", raw.timeout))?;
            Ok(McpServer {
                name: raw.name,
                command: raw.command,
                args: raw.args,
                enabled: raw.enabled,
                allow_tools: raw.allow_tools,
                timeout,
                scope,
            })
        })
        .collect()
}

/// A stable id for a server's command line, used for approval of repository servers.
pub fn fingerprint(server: &McpServer) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"mcp\0");
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

pub fn is_trusted(server: &McpServer, approved: &[String]) -> bool {
    server.scope == Scope::User || approved.iter().any(|fp| *fp == fingerprint(server))
}

/// Only tools named in the server's `allow_tools` survive. Everything else is invisible.
pub fn allowed(server: &McpServer, tools: Vec<Tool>) -> Vec<Tool> {
    tools
        .into_iter()
        .filter(|tool| server.allow_tools.contains(&tool.name))
        .collect()
}

/// The JSON-RPC conversation over any reader and writer, so tests can use in-memory pipes.
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
        let mut line = message.to_string();
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .and_then(|()| self.writer.flush())
            .map_err(|err| format!("mcp write: {err}"))
    }

    /// Sends a request and returns its `result`. Notifications and replies to other ids are skipped.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .map_err(|err| format!("mcp read: {err}"))?;
            if read == 0 {
                return Err(format!("mcp server closed the connection during {method}"));
            }
            let Ok(message) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if message.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(format!("{method}: {error}"));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "plaesy", "version": env!("CARGO_PKG_VERSION")}
            }),
        )?;
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
    }

    pub fn list_tools(&mut self) -> Result<Vec<Tool>, String> {
        let result = self.request("tools/list", json!({}))?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| "tools/list: no tools array".to_owned())?;
        Ok(tools
            .iter()
            .filter_map(|tool| {
                Some(Tool {
                    name: tool.get("name")?.as_str()?.to_owned(),
                    description: tool
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                })
            })
            .collect())
    }

    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String, String> {
        let result = self.request("tools/call", json!({"name": name, "arguments": arguments}))?;
        let text: Vec<&str> = result
            .get("content")
            .and_then(Value::as_array)
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|part| part.get("text").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();
        let joined = text.join("\n");
        if result.get("isError").and_then(Value::as_bool) == Some(true) {
            Err(joined)
        } else {
            Ok(joined)
        }
    }
}

/// Starts the server, runs `work` on it, and stops the server. The server is killed when it does
/// not answer within the timeout, so a hung server cannot block the chat.
pub fn with_server<T, F>(server: &McpServer, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(
            &mut Client<BufReader<std::process::ChildStdout>, std::process::ChildStdin>,
        ) -> Result<T, String>
        + Send
        + 'static,
{
    let mut child = Command::new(&server.command)
        .args(&server.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("mcp {} ({}): {err}", server.name, server.command))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("mcp {}: no stdin", server.name))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("mcp {}: no stdout", server.name))?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut client = Client::new(BufReader::new(stdout), stdin);
        let result = client.initialize().and_then(|()| work(&mut client));
        let _ = sender.send(result);
    });
    let outcome = receiver.recv_timeout(server.timeout);
    let _ = child.kill();
    let _ = child.wait();
    match outcome {
        Ok(result) => result,
        Err(_) => Err(format!(
            "mcp {} did not answer within {:?}",
            server.name, server.timeout
        )),
    }
}

/// `/mcp`: each enabled, trusted server with its allowed tools, or the reason it is unavailable.
pub fn list_all(servers: &[McpServer], approved: &[String]) -> String {
    if servers.is_empty() {
        return "no mcp servers".to_owned();
    }
    let mut lines = Vec::new();
    for server in servers.iter().filter(|s| s.enabled) {
        if !is_trusted(server, approved) {
            lines.push(format!(
                "{}: not approved; add {} to approved-hooks.json to run it",
                server.name,
                fingerprint(server)
            ));
            continue;
        }
        match with_server(server, move |client| client.list_tools()) {
            Ok(tools) => {
                let visible = allowed(server, tools);
                let names: Vec<String> = visible.iter().map(|t| t.name.clone()).collect();
                lines.push(format!(
                    "{}: {}",
                    server.name,
                    if names.is_empty() {
                        "no allowed tools".to_owned()
                    } else {
                        names.join(", ")
                    }
                ));
            }
            Err(err) => lines.push(format!("{}: unavailable ({err})", server.name)),
        }
    }
    lines.join("\n")
}

/// `/mcp call <server> <tool> <json>`: refused unless the tool is allowed for that server.
pub fn call(
    servers: &[McpServer],
    approved: &[String],
    server_name: &str,
    tool: &str,
    arguments: Value,
) -> Result<String, String> {
    let server = servers
        .iter()
        .find(|s| s.name == server_name)
        .ok_or_else(|| format!("no mcp server {server_name}"))?;
    if !server.enabled {
        return Err(format!("mcp server {server_name} is disabled"));
    }
    if !is_trusted(server, approved) {
        return Err(format!("mcp server {server_name} is not approved"));
    }
    if !server.allow_tools.iter().any(|name| name == tool) {
        return Err(format!(
            "tool {tool} is not allowed for mcp server {server_name}"
        ));
    }
    let tool_name = tool.to_owned();
    with_server(server, move |client| {
        client.call_tool(&tool_name, arguments)
    })
}

/// Reads approvals (JSON array of fingerprints). Missing file means none.
pub fn load_approved(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "chat_mcp_test.rs"]
mod chat_mcp_test;
