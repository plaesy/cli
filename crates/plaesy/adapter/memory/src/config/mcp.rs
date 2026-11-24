use serde_json::{json, Value};
use std::collections::BTreeMap;

pub async fn run(
    app: Option<String>,
    list: bool,
    binary: String,
    notes_dir: String,
    http_url: Option<String>,
) -> crate::memory::Result<()> {
    let snippets = all_snippets(&binary, &notes_dir, &http_url);

    if list {
        for (name, snippet) in &snippets {
            println!("=== {} ===", name);
            println!("{}", snippet);
            println!();
        }
        return Ok(());
    }

    match app {
        Some(ref name) => {
            if let Some(snippet) = snippets.get(name.as_str()) {
                println!("{}", snippet);
            } else {
                eprintln!("error: unknown app '{name}'");
                eprintln!(
                    "available: {}",
                    snippets
                        .keys()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                std::process::exit(2);
            }
        }
        None => {
            println!(
                "Usage: plaesy memory config mcp --app <name> [--binary BIN] [--notes-dir DIR] [--http-url URL]"
            );
            println!();
            println!("Available apps:");
            for name in snippets.keys() {
                println!("  * {name}");
            }
            println!();
            println!("Print all snippets: plaesy memory config mcp --list");
        }
    }

    Ok(())
}

fn all_snippets(
    binary: &str,
    notes_dir: &str,
    http_url: &Option<String>,
) -> BTreeMap<String, String> {
    let mut m: BTreeMap<String, String> = BTreeMap::new();

    m.insert(
        "claude-code".to_string(),
        claude_code(binary, notes_dir, http_url),
    );
    m.insert(
        "kilocode".to_string(),
        kilocode(binary, notes_dir, http_url),
    );
    m.insert("cursor".to_string(), cursor(binary, notes_dir, http_url));
    m.insert("n8n".to_string(), n8n(binary, notes_dir, http_url));
    m.insert("dify".to_string(), dify(binary, notes_dir, http_url));
    m.insert("crewai".to_string(), crewai(binary, notes_dir, http_url));
    m.insert(
        "paperclip".to_string(),
        paperclip(binary, notes_dir, http_url),
    );
    m.insert("hermes".to_string(), hermes(binary, notes_dir, http_url));
    m.insert("codex".to_string(), codex(binary, notes_dir, http_url));

    m
}

fn stdio_config(binary: &str, notes_dir: &str) -> Value {
    let mut config = json!({
        "type": "stdio",
        "command": binary,
        "args": ["memory", "serve", "--mcp"]
    });
    if !notes_dir.is_empty() {
        config["env"] = json!({"MEMORY_WATCH_DIR": notes_dir});
    }
    config
}

fn http_config(http_url: &Option<String>) -> Value {
    match http_url {
        Some(url) => json!({
            "url": url,
            "headers": {}
        }),
        None => json!({
            "url": "http://127.0.0.1:8080/mcp",
            "headers": {}
        }),
    }
}

fn claude_code(binary: &str, notes_dir: &str, http_url: &Option<String>) -> String {
    let mut cfg = http_config(http_url);
    cfg["type"] = json!("http");
    let stdio_cfg = stdio_config(binary, notes_dir);
    format!(
        r#"# Claude Code — Plaesy Memory MCP
# Add to .mcp.json, or use: claude mcp add --transport http plaesy-memory {url}

{{
  "mcpServers": {{
    "plaesy-memory": {}
  }}
}}

# Stdio alternative (use a separate vault when another server is running):
{{
  "mcpServers": {{
    "plaesy-memory": {}
  }}
}}

"#,
        pretty_json(&cfg),
        pretty_json(&stdio_cfg),
        url = http_url.as_deref().unwrap_or("http://127.0.0.1:8080/mcp")
    )
}

/// Pretty-print a config snippet. Our config structs contain only strings and maps, so
/// serialization cannot fail; the `expect` documents that invariant instead of hiding it.
fn pretty_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("MCP config snippet is always serializable")
}

fn kilocode(binary: &str, notes_dir: &str, http_url: &Option<String>) -> String {
    let url = http_url.as_deref().unwrap_or("http://127.0.0.1:8080/mcp");
    let mut local = json!({
        "type": "local",
        "command": [binary, "memory", "serve", "--mcp"],
        "enabled": true
    });
    if !notes_dir.is_empty() {
        local["environment"] = json!({"MEMORY_WATCH_DIR": notes_dir});
    }
    format!(
        r#"# Kilo CLI: add to kilo.jsonc or .kilo/kilo.jsonc

{{
  "mcp": {{
    "plaesy-memory": {{ "type": "remote", "url": {url:?}, "enabled": true }}
  }}
}}

# Local alternative (use a separate vault when another server is running):
{{
  "mcp": {{
    "plaesy-memory": {}
  }}
}}
"#,
        pretty_json(&local)
    )
}

fn cursor(binary: &str, notes_dir: &str, http_url: &Option<String>) -> String {
    let cfg = http_config(http_url);
    let stdio_cfg = stdio_config(binary, notes_dir);
    format!(
        r#"# Cursor — Plaesy Memory MCP
# Add to: ~/.cursor/mcp.json

{{
  "mcpServers": {{
    "plaesy-memory": {}
  }}
}}

# Stdio alternative:
{{
  "mcpServers": {{
    "plaesy-memory": {}
  }}
}}

# Then run: cursor mcp add plaesy-memory
"#,
        pretty_json(&cfg),
        pretty_json(&stdio_cfg)
    )
}

fn n8n(binary: &str, _notes_dir: &str, http_url: &Option<String>) -> String {
    format!(
        r#"# n8n — Plaesy Memory MCP
# Option A: MCP Server Trigger node (n8n is the MCP server host)
# In n8n, add "MCP Server Trigger" and configure it to run:
#   {binary} memory serve --mcp --transport http --port 8080
# Then tools "search", "stats", "links" are available in workflows.

# Option B: MCP Client node (n8n calls remote MCP server)
# Set "Server URL" to:
#   {url}
# Available tools: memory_search, memory_stats, memory_links

# Start the server first:
#   {binary} memory serve --mcp --transport http --port 8080
"#,
        binary = binary,
        url = http_url.as_deref().unwrap_or("http://127.0.0.1:8080/mcp")
    )
}

fn dify(binary: &str, _notes_dir: &str, http_url: &Option<String>) -> String {
    format!(
        r#"# Dify — Plaesy Memory MCP
# In Dify: Knowledge / MCP Server / Add MCP Server
# Select "Streamable HTTP" transport.
# Server URL:
#   {url}
# Available tools: memory_search, memory_stats, memory_links
#
# Start the server first:
#   {binary} memory serve --mcp --transport http --port 8080
"#,
        binary = binary,
        url = http_url.as_deref().unwrap_or("http://127.0.0.1:8080/mcp")
    )
}

fn crewai(binary: &str, _notes_dir: &str, _http_url: &Option<String>) -> String {
    format!(
        r#"# CrewAI — Plaesy Memory MCP
# pip install crewai-tools[mcp]

from crewai_tools import MCPServerAdapter
from crewai import Agent, Task, Crew

# Stdio transport:
with MCPServerAdapter({{ command: "{binary}", args: ["memory", "serve", "--mcp"] }}) as mcp:
    agent = Agent(role="Researcher", tools=mcp.tools)
    ...
"#,
        binary = binary
    )
}

fn paperclip(binary: &str, _notes_dir: &str, _http_url: &Option<String>) -> String {
    format!(
        r#"# Paperclip / Hindsight — Plaesy Memory MCP
# Paperclip uses @vectorize-io/hindsight-paperclip plugin.
# Configure in paperclip.config.ts:

import {{ HindsightPaperclip }} from "@vectorize-io/hindsight-paperclip";

export default {{
  plugins: [
    new HindsightPaperclip({{
      mcpCommand: "{binary}",
      mcpArgs: ["memory", "serve", "--mcp"],
    }}),
  ],
}};
"#,
        binary = binary
    )
}

fn hermes(binary: &str, _notes_dir: &str, http_url: &Option<String>) -> String {
    let url = http_url.as_deref().unwrap_or("http://127.0.0.1:8080/mcp");
    format!(
        r#"# Hermes: add to ~/.hermes/config.yaml under mcp_servers:
mcp_servers:
  plaesy-memory:
    url: {url}

# Start one shared HTTP server before connecting multiple clients.
# Stdio alternative for a separate vault:
#   command: {binary}
#   args: ["memory", "serve", "--mcp"]
"#
    )
}

fn codex(binary: &str, _notes_dir: &str, http_url: &Option<String>) -> String {
    let url = http_url.as_deref().unwrap_or("http://127.0.0.1:8080/mcp");
    format!(
        r#"# Codex: add to ~/.codex/config.toml
[mcp_servers.plaesy-memory]
url = {url:?}

# Or use the CLI: codex mcp add plaesy-memory --url {url}
# Start one shared HTTP server before connecting multiple clients.
# Stdio alternative for a separate vault:
#   codex mcp add plaesy-memory -- {binary} memory serve --mcp
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_snippet_never_switches_to_http() {
        let config = stdio_config("plaesy", "/notes");
        assert_eq!(config["args"], json!(["memory", "serve", "--mcp"]));
        assert_eq!(config["env"]["MEMORY_WATCH_DIR"], "/notes");
    }

    #[test]
    fn stdio_snippet_uses_configured_vault_by_default() {
        let config = stdio_config("plaesy", "");
        assert!(config.get("env").is_none());
    }

    #[test]
    fn codex_and_hermes_snippets_use_the_requested_url() {
        let url = Some("http://localhost:9123/mcp".into());
        assert!(codex("plaesy", "/notes", &url).contains("url = \"http://localhost:9123/mcp\""));
        assert!(hermes("plaesy", "/notes", &url).contains("url: http://localhost:9123/mcp"));
    }

    #[test]
    fn claude_http_snippet_uses_claude_transport_name() {
        let snippet = claude_code("plaesy", "/notes", &None);
        assert!(snippet.contains("\"type\": \"http\""));
        assert!(!snippet.contains("\"type\": \"streamable_http\""));
    }

    #[test]
    fn kilo_cli_snippet_uses_current_config_shape() {
        let snippet = kilocode("plaesy", "", &None);
        assert!(snippet.contains("\"mcp\": {"));
        assert!(snippet.contains("\"type\": \"remote\""));
        assert!(snippet.contains("\"type\": \"local\""));
        assert!(!snippet.contains("\"mcpServers\""));
    }

    #[test]
    fn cursor_http_snippet_uses_url_without_unrecognized_type() {
        let config = http_config(&None);
        assert_eq!(config["url"], "http://127.0.0.1:8080/mcp");
        assert!(config.get("type").is_none());
    }
}
