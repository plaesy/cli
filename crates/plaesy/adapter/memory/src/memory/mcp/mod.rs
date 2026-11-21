//! Model Context Protocol server.
//!
//! Supports two transports: stdio (newline-delimited JSON-RPC 2.0) and HTTP
//! Streamable HTTP (JSON request-response), so any MCP client
//! (Claude Code / Desktop, Cursor, Hermes, n8n, Dify, Kilo Code, …) can use the
//! memory without a per-client shim. Read tools query indexed notes;
//! `memory_remember` creates a note and `memory_resolve_gap` closes a query gap.
//! The index is built and kept current by background sync of `memory serve --mcp`.
//!
//! Protocol support: MCP 2026-07-28 and compatible handshake-era clients through 2024-11-05,
//! provided by the official Rust MCP SDK.
//!
//! Transports:
//! - `stdio` (default): `memory serve --mcp`
//! - `http`: `memory serve --mcp --transport http --port 8080`

use crate::memory::error::{MemoryError, Result};
use crate::memory::hygiene::HygieneOptions;
use crate::memory::search::{SearchEngine, SearchMode};
use crate::memory::service::{parse_filter, SearchHit};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
#[cfg(test)]
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

#[cfg(test)]
const LATEST: &str = "2025-11-25";
#[cfg(test)]
const SUPPORTED: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

fn tools() -> Value {
    let read_only =
        json!({ "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false });
    // Creates a file or closes a gap; never overwrites or deletes anything.
    let write_safe = json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false });
    json!([
        {
            "name": "memory_search",
            "description": "Search the project's indexed memory notes. Returns verbatim passages, each with a citation (file, line range, git revision, index time), an evidence label (FACT = lexical match in the source, INFERRED = found by meaning only) and what the source declares about itself (tier, trust, last verified, STALE). Prefer FACT hits, treat STALE or low-trust hits as leads to verify, and cite the file:lines when you use a passage.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to look for, in natural language or keywords." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 5 },
                    "mode": { "type": "string", "enum": ["hybrid", "semantic", "keyword"], "default": "hybrid" },
                    "tier": {
                        "description": "Restrict to tiers: core, episodic, semantic, procedural, archive.",
                        "oneOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }]
                    },
                    "path_prefix": { "type": "string", "description": "Only notes whose path starts with this." },
                    "min_trust": { "type": "string", "enum": ["high", "medium", "low"], "description": "Only notes declaring at least this trust." },
                    "include_inactive": { "type": "boolean", "default": false, "description": "Also return superseded / deprecated notes." },
                    "as_of": { "type": "string", "format": "date", "description": "Only notes whose valid_from..valid_to period contains this date (YYYY-MM-DD). Undated notes always pass." },
                    "expand_links": { "type": "boolean", "default": false, "description": "Also add the best passage of notes linked to or from the top hits (one hop)." },
                    "explain": { "type": "boolean", "default": false, "description": "Include per-leg score breakdown (fts, vector, fused, staleness, rerank)." }
                },
                "required": ["query"]
            },
            "outputSchema": {
                "type": "object",
                "properties": {
                    "hits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "chunk_id": { "type": "integer" },
                                "text": { "type": "string" },
                                "score": { "type": "number" },
                                "confidence": { "type": "string", "enum": ["FACT", "INFERRED", "SPECULATIVE"] },
                                "citation": {
                                    "type": "object",
                                    "properties": {
                                        "docid": { "type": "string" },
                                        "file_path": { "type": "string" },
                                        "revision": { "type": ["string", "null"] },
                                        "line_range": { "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2 },
                                        "timestamp": { "type": "string" }
                                    },
                                    "required": ["docid", "file_path", "line_range", "timestamp"]
                                }
                            },
                            "required": ["chunk_id", "text", "score", "confidence", "citation"]
                        }
                    }
                },
                "required": ["hits"]
            },
            "annotations": read_only
        },
        {
            "name": "memory_backlinks",
            "description": "List [[wikilinks]] for a note: the notes that link to it (default) or the notes it links to. Dangling links have no resolved_path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Note path, path without extension, or bare file name." },
                    "direction": { "type": "string", "enum": ["incoming", "outgoing"], "default": "incoming" }
                },
                "required": ["path"]
            },
            "annotations": read_only
        },
        {
            "name": "memory_hygiene",
            "description": "Audit the memory for problems: stale notes, missing last_verified/confidence on claim-bearing tiers, superseded notes without a replacement, broken wikilinks, and backticked file paths that no longer exist. Read-only.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": read_only
        },
        {
            "name": "memory_missing",
            "description": "What the memory is missing: questions past searches could not answer (most pressing first) and structural gaps (broken wikilinks, stale notes). Read-only; this tool does not record queries.",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 } }
            },
            "annotations": read_only
        },
        {
            "name": "memory_curate",
            "description": "Prepare a read-only memory maintenance review combining hygiene findings, unanswered searches, exact duplicates, unlinked-note candidates, and knowledge folders that may need an index. It never edits or deletes vault notes.",
            "inputSchema": {
                "type": "object",
                "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 } }
            },
            "annotations": read_only
        },
        {
            "name": "memory_remember",
            "description": "Save a new note for the memory: a Markdown file under the notes root, indexed at once so it can be cited. Creates files only; an existing note is never overwritten, and folders must stay inside the notes root. Notes written here are marked source: agent, and a confidence of high is stored as medium: the owner raises it by hand.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "folder": { "type": "string", "description": "Relative folder under the notes root. Default: the vault's inbox folder (for example 00. Inbox), else inbox." },
                    "tier": { "type": "string", "enum": ["core", "episodic", "semantic", "procedural", "archive"], "default": "semantic" },
                    "confidence": { "type": "string", "enum": ["high", "medium", "low"], "default": "medium", "description": "high is stored as medium." }
                },
                "required": ["title", "body"]
            },
            "annotations": write_safe
        },
        {
            "name": "memory_resolve_gap",
            "description": "Close an open query gap from memory_missing once the question has been answered. It reopens if the same query misses again.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer" } },
                "required": ["id"]
            },
            "annotations": write_safe
        },
        {
            "name": "memory_stats",
            "description": "Index statistics: documents, chunks, embeddings, and whether semantic search is available.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": read_only
        }
    ])
}

#[cfg(test)]
fn rpc_ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

#[cfg(test)]
fn rpc_err(id: &Value, code: i64, message: impl Into<String>) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } })
}

fn tool_text(text: String, structured: Value) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "structuredContent": structured, "isError": false })
}

fn tool_error(message: impl Into<String>) -> Value {
    json!({ "content": [{ "type": "text", "text": message.into() }], "isError": true })
}

/// Render hits for a model: one block per hit with its citation first.
fn render_hits(hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return "No matching notes.".to_string();
    }
    let mut out = String::new();
    for (i, h) in hits.iter().enumerate() {
        out.push_str(&format!(
            "{}. score {:.2}  {}\n",
            i + 1,
            h.score,
            h.citation
        ));
        for line in h.text.lines() {
            out.push_str("   > ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
async fn call_tool(engine: &SearchEngine, name: &str, args: &Value) -> Value {
    match run_tool(engine, name, args).await {
        Ok(v) => v,
        Err(ToolError::Invalid(m)) => tool_error(format!("invalid arguments: {m}")),
        Err(ToolError::Engine(e)) => tool_error(format!("{e}")),
    }
}

enum ToolError {
    Invalid(String),
    Engine(MemoryError),
}

impl From<MemoryError> for ToolError {
    fn from(e: MemoryError) -> Self {
        ToolError::Engine(e)
    }
}

async fn run_tool(
    engine: &SearchEngine,
    name: &str,
    args: &Value,
) -> std::result::Result<Value, ToolError> {
    match name {
        "memory_search" => {
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .filter(|q| !q.trim().is_empty())
                .ok_or_else(|| {
                    ToolError::Invalid("`query` (non-empty string) is required".into())
                })?;
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map_or(5, |n| n.clamp(1, 50) as usize);
            let mode_name = args.get("mode").and_then(Value::as_str).unwrap_or("hybrid");
            let mode = SearchMode::parse(mode_name)
                .ok_or_else(|| ToolError::Invalid(format!("unknown mode '{mode_name}'")))?;
            let filter = parse_filter(args).map_err(ToolError::Invalid)?;
            let expand = args
                .get("expand_links")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let explain = args
                .get("explain")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let results = if expand {
                engine
                    .search_expanded_with_explain(query, limit, mode, &filter, explain)
                    .await?
            } else if explain {
                engine
                    .search_with_explain(query, limit, mode, &filter, explain)
                    .await?
            } else {
                engine.search_with(query, limit, mode, &filter).await?
            };
            let hits: Vec<SearchHit> = results
                .iter()
                .map(SearchHit::from_result)
                .collect::<Result<_>>()?;
            Ok(tool_text(render_hits(&hits), json!({ "hits": hits })))
        }
        "memory_backlinks" => {
            let path = args
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::Invalid("`path` (string) is required".into()))?;
            let incoming = args
                .get("direction")
                .and_then(Value::as_str)
                .is_none_or(|d| d != "outgoing");
            let links = engine.links(path, incoming).await?;
            let text = if links.is_empty() {
                "No links.".to_string()
            } else {
                links
                    .iter()
                    .map(|l| {
                        format!(
                            "{}:{}  [[{}]] -> {}",
                            l.source_path,
                            l.line,
                            l.target,
                            l.resolved_path.as_deref().unwrap_or("(unresolved)")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(tool_text(text, json!({ "links": links })))
        }
        "memory_hygiene" => {
            let report = engine.hygiene(&HygieneOptions::default()).await?;
            let mut text = format!(
                "{} notes checked, {} findings\n",
                report.checked,
                report.findings.len()
            );
            for f in &report.findings {
                text.push_str(&format!(
                    "- [{}] {} {}{}: {}\n",
                    f.label,
                    f.kind,
                    f.path,
                    f.line.map(|l| format!(":{l}")).unwrap_or_default(),
                    f.detail
                ));
            }
            let structured = serde_json::to_value(&report)
                .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
            Ok(tool_text(text, structured))
        }
        "memory_missing" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(20)
                .clamp(1, 100) as usize;
            let report = engine.missing(None, limit).await?;
            let mut text = format!(
                "{} open query gaps, {} structural gaps\n",
                report.query_gaps.len(),
                report.structural.len()
            );
            for g in &report.query_gaps {
                text.push_str(&format!("- #{} x{} {}\n", g.id, g.misses, g.query));
            }
            for f in &report.structural {
                text.push_str(&format!(
                    "- [{}] {} {}: {}\n",
                    f.label, f.kind, f.path, f.detail
                ));
            }
            let structured = serde_json::to_value(&report)
                .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
            Ok(tool_text(text, structured))
        }
        "memory_curate" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(20)
                .clamp(1, 100) as usize;
            let options = HygieneOptions {
                root: engine.watch_root().map(std::path::Path::to_path_buf),
                ..HygieneOptions::default()
            };
            let report = engine.curate(&options, limit).await?;
            let text = format!(
                "Read-only curation: {} notes; {} hygiene findings ({} confirmed, {} inferred); {} query gaps; {} exact duplicate groups; {} unlinked candidates; {} topic-index candidates. No notes were changed.",
                report.checked,
                report.hygiene_findings_total,
                report.confirmed_findings,
                report.inferred_findings,
                report.query_gaps.items.len(),
                report.exact_duplicates.items.len(),
                report.unlinked_notes.items.len(),
                report.missing_topic_indexes.items.len()
            );
            let structured = serde_json::to_value(&report)
                .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
            Ok(tool_text(text, structured))
        }
        "memory_remember" => {
            let text_arg = |key: &str| {
                args.get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            let note = crate::memory::notes::NewNote {
                folder: text_arg("folder"),
                title: text_arg("title"),
                body: text_arg("body"),
                tier: args
                    .get("tier")
                    .and_then(Value::as_str)
                    .unwrap_or("semantic")
                    .to_string(),
                confidence: args
                    .get("confidence")
                    .and_then(Value::as_str)
                    .unwrap_or("medium")
                    .to_string(),
                graph: None,
            };
            if note.title.trim().is_empty() || note.body.trim().is_empty() {
                return Err(ToolError::Invalid(
                    "title and body must not be empty".into(),
                ));
            }
            let path = engine.remember(&note).await?;
            let text = format!("saved {}", path.display());
            Ok(tool_text(
                text,
                json!({ "path": path.display().to_string() }),
            ))
        }
        "memory_resolve_gap" => {
            let id = args
                .get("id")
                .and_then(Value::as_i64)
                .ok_or_else(|| ToolError::Invalid("`id` (integer) is required".into()))?;
            let resolved = engine.resolve_gap(id)?;
            let text = if resolved {
                format!("resolved gap {id}")
            } else {
                format!("no open gap with id {id}")
            };
            Ok(tool_text(text, json!({ "resolved": resolved })))
        }
        "memory_stats" => {
            let s = engine.statistics().await?;
            let mut v = serde_json::to_value(&s)
                .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
            if let Some(o) = v.as_object_mut() {
                o.insert("semantic_search".into(), json!(engine.has_vector_search()));
            }
            Ok(tool_text(v.to_string(), v))
        }
        other => Err(ToolError::Invalid(format!("unknown tool '{other}'"))),
    }
}

/// Handle one JSON-RPC message. Returns `None` for notifications (nothing to send back).
#[cfg(test)]
pub async fn handle_message(engine: &SearchEngine, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str);
    let Some(method) = method else {
        // A response or garbage: servers that never send requests have nothing to do with it.
        return id.map(|id| rpc_err(&id, -32600, "invalid request: missing method"));
    };
    let Some(id) = id else {
        return None; // notification (e.g. notifications/initialized)
    };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(LATEST);
            let version = if SUPPORTED.contains(&asked) {
                asked
            } else {
                LATEST
            };
            rpc_ok(
                &id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "plaesy-memory", "version": env!("CARGO_PKG_VERSION") },
                    "instructions": "Memory over the project's notes. Use memory_search before answering from memory; cite the returned file:lines; treat STALE or INFERRED hits as unverified. memory_curate is a read-only maintenance report. Writes are limited to memory_remember (new notes only, never overwrites) and memory_resolve_gap."
                }),
            )
        }
        "ping" => rpc_ok(&id, json!({})),
        "tools/list" => rpc_ok(&id, json!({ "tools": tools() })),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(rpc_err(&id, -32602, "tools/call requires params.name"));
            };
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            rpc_ok(&id, call_tool(engine, name, &args).await)
        }
        other => rpc_err(&id, -32601, format!("method not found: {other}")),
    })
}

/// Serve MCP over `input`/`output` until EOF (one JSON message per line).
///
/// # Errors
///
/// - `IoError` if reading or writing the streams fails
#[cfg(test)]
pub async fn serve<R, W>(engine: &SearchEngine, input: R, mut output: W) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let io_err = |e: std::io::Error| MemoryError::IoError {
        path: "<stdio>".to_string(),
        source: e,
    };
    let mut lines = BufReader::new(input).lines();
    while let Some(line) = lines.next_line().await.map_err(io_err)? {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle_message(engine, &msg).await,
            Err(e) => Some(rpc_err(&Value::Null, -32700, format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            let mut bytes = serde_json::to_vec(&reply)
                .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
            bytes.push(b'\n');
            output.write_all(&bytes).await.map_err(io_err)?;
            output.flush().await.map_err(io_err)?;
        }
    }
    Ok(())
}

/// Serve MCP over HTTP Streamable transport (POST /mcp for JSON-RPC).
/// This supports n8n, Dify, Kilo Code, Paperclip, and other HTTP-based MCP clients.
pub async fn serve_stdio(engine: Arc<SearchEngine>) -> Result<()> {
    let service = PlaesyMcpServer { engine }
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| MemoryError::ConfigError(format!("MCP stdio initialization failed: {e}")))?;
    service
        .waiting()
        .await
        .map_err(|e| MemoryError::ConfigError(format!("MCP stdio service failed: {e}")))?;
    Ok(())
}

async fn require_bearer_token(
    axum::extract::State(expected_token): axum::extract::State<String>,
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::header::AUTHORIZATION;
    use axum::response::IntoResponse;

    let authorized = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|provided| constant_time_eq(provided.as_bytes(), expected_token.as_bytes()));
    if authorized {
        next.run(request).await
    } else {
        axum::http::StatusCode::UNAUTHORIZED.into_response()
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

#[test]
fn bearer_token_comparison_requires_exact_bytes() {
    assert!(constant_time_eq(
        b"long-token-value-1234567890",
        b"long-token-value-1234567890"
    ));
    assert!(!constant_time_eq(
        b"long-token-value-1234567890",
        b"long-token-value-1234567891"
    ));
    assert!(!constant_time_eq(b"long-token-value-1234567890", b"short"));
    assert!(!constant_time_eq(b"", b"long-token-value-1234567890"));
}

pub async fn serve_http(
    engine: Arc<SearchEngine>,
    addr: SocketAddr,
    auth_token: Option<String>,
) -> Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let allowed_origins = [
        "http://localhost:*",
        "https://localhost:*",
        "http://127.0.0.1:*",
        "https://127.0.0.1:*",
        "http://[::1]:*",
        "https://[::1]:*",
    ];
    let config = StreamableHttpServerConfig::default()
        .with_allowed_origins(allowed_origins)
        .enforce_origin_validation()
        .with_json_response(true);
    let service = StreamableHttpService::new(
        move || {
            Ok(PlaesyMcpServer {
                engine: Arc::clone(&engine),
            })
        },
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let app = axum::Router::new().nest_service("/mcp", service);
    let app = if let Some(expected_token) = auth_token {
        app.route_layer(axum::middleware::from_fn_with_state(
            expected_token,
            require_bearer_token,
        ))
    } else {
        app
    };

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| MemoryError::IoError {
            path: addr.to_string(),
            source: e,
        })?;

    tracing::info!("MCP HTTP server listening on http://{}", addr);
    axum::serve(listener, app)
        .await
        .map_err(|e| MemoryError::IoError {
            path: addr.to_string(),
            source: e,
        })
}

#[derive(Clone)]
struct PlaesyMcpServer {
    engine: Arc<SearchEngine>,
}

impl ServerHandler for PlaesyMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "plaesy-memory",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("Use memory_search before answering from memory; cite the returned file:lines; treat STALE or INFERRED hits as unverified. memory_curate is a read-only maintenance report. Writes are limited to memory_remember (new notes only, never overwrites) and memory_resolve_gap.")
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = std::result::Result<ListToolsResult, McpError>> + Send + '_
    {
        std::future::ready(
            serde_json::from_value::<Vec<Tool>>(tools())
                .map(ListToolsResult::with_all_items)
                .map_err(|e| {
                    McpError::internal_error(format!("invalid MCP tool schema: {e}"), None)
                }),
        )
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        serde_json::from_value::<Vec<Tool>>(tools())
            .ok()?
            .into_iter()
            .find(|tool| tool.name == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResponse, McpError> {
        let args = request
            .arguments
            .map(Value::Object)
            .unwrap_or_else(|| json!({}));
        let result = match run_tool(&self.engine, &request.name, &args).await {
            Ok(result) => result,
            Err(ToolError::Invalid(message)) => tool_error(format!("invalid arguments: {message}")),
            Err(ToolError::Engine(error)) => tool_error(error.to_string()),
        };
        let result: CallToolResult = serde_json::from_value(result)
            .map_err(|e| McpError::internal_error(format!("invalid MCP tool result: {e}"), None))?;
        Ok(CallToolResponse::Complete(result))
    }
}

#[cfg(test)]
#[path = "mcp_test.rs"]
mod mcp_test;
