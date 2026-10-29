//! A minimal MCP client over stdio. Each call starts the server, sends one JSON-RPC request,
//! reads one response, and stops the server.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use super::exec::{argv, capture, Deadline};
use super::ReachError;

/// The text a tool call returned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpResult {
    pub text: String,
}

/// A tool a server offers.
#[derive(Debug, Clone, PartialEq)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// One MCP server, started on demand.
#[derive(Debug, Clone)]
pub struct McpClient {
    server_name: String,
    command: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

impl McpClient {
    #[must_use]
    pub fn new(server_name: &str, command: &str, args: &[&str]) -> Self {
        Self {
            server_name: server_name.to_owned(),
            command: command.to_owned(),
            args: args.iter().map(|s| (*s).to_owned()).collect(),
            env: Vec::new(),
        }
    }

    /// Sets an environment variable for the server process.
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.push((key.to_owned(), value.to_owned()));
    }

    /// Calls `tool` with `params`, and joins the text parts of the result, one line each.
    ///
    /// # Errors
    ///
    /// The server cannot be run, fails, or answers with an error.
    pub fn call(
        &self,
        tool: &str,
        params: Map<String, Value>,
        deadline: Deadline,
    ) -> Result<McpResult, ReachError> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": tool, "arguments": params},
        });
        let result = self.exchange(&request, deadline)?;
        let mut text = String::new();
        if let Some(items) = result.get("content").and_then(Value::as_array) {
            for item in items {
                if let Some(t) = item.get("text").and_then(Value::as_str) {
                    text.push_str(t);
                    text.push('\n');
                }
            }
        }
        Ok(McpResult { text })
    }

    /// # Errors
    ///
    /// The server cannot be run, fails, or answers with an error.
    pub fn list_tools(&self, deadline: Deadline) -> Result<Vec<McpTool>, ReachError> {
        let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
        let result = self.exchange(&request, deadline)?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|t| McpTool {
                        name: t
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        description: t
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        input_schema: t.get("inputSchema").cloned().unwrap_or(Value::Null),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(tools)
    }

    /// Sends one request and returns the response's `result`.
    fn exchange(&self, request: &Value, deadline: Deadline) -> Result<Value, ReachError> {
        let body = serde_json::to_vec(request)?;
        let mut command = argv(&[self.command.as_str()]);
        command.extend(self.args.iter().cloned());
        let out = capture(&command, &self.env, Some(&body), deadline).map_err(|e| {
            ReachError::Mcp(format!("starting MCP server {}: {e}", self.server_name))
        })?;
        if !out.status.success() {
            return Err(ReachError::Mcp(format!(
                "MCP server error: {}: {}",
                self.server_name,
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let response: Value = serde_json::from_slice(&out.stdout)?;
        if let Some(err) = response.get("error") {
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error");
            return Err(ReachError::Mcp(format!("MCP error: {message}")));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }
}

/// Named MCP servers.
#[derive(Default)]
pub struct McpManager {
    clients: HashMap<String, McpClient>,
}

impl McpManager {
    pub fn register(&mut self, name: &str, client: McpClient) {
        self.clients.insert(name.to_owned(), client);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&McpClient> {
        self.clients.get(name)
    }

    /// # Errors
    ///
    /// The server is not registered, or the call fails.
    pub fn call(
        &self,
        server: &str,
        tool: &str,
        params: Map<String, Value>,
        deadline: Deadline,
    ) -> Result<McpResult, ReachError> {
        self.require(server)?.call(tool, params, deadline)
    }

    /// # Errors
    ///
    /// The server is not registered, or listing fails.
    pub fn list_tools(&self, server: &str, deadline: Deadline) -> Result<Vec<McpTool>, ReachError> {
        self.require(server)?.list_tools(deadline)
    }

    fn require(&self, server: &str) -> Result<&McpClient, ReachError> {
        self.get(server)
            .ok_or_else(|| ReachError::Mcp(format!("MCP server {server} not registered")))
    }
}

/// The servers reached through mcporter.
#[must_use]
pub fn default_manager() -> McpManager {
    let mut m = McpManager::default();
    for name in ["exa", "linkedin", "xiaohongshu", "xiaoyuzhou"] {
        m.register(name, McpClient::new(name, "mcporter", &[name]));
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_manager_knows_the_four_mcporter_servers() {
        let m = default_manager();
        for name in ["exa", "linkedin", "xiaohongshu", "xiaoyuzhou"] {
            assert!(m.get(name).is_some(), "{name}");
        }
        assert!(m.get("missing").is_none());
    }

    #[test]
    fn calling_an_unregistered_server_is_an_error() {
        let m = McpManager::default();
        let deadline = Deadline::after(std::time::Duration::from_secs(1));
        assert!(m.call("nope", "x", Map::new(), deadline).is_err());
    }
}
