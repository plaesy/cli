//! Line-delimited JSON service protocol used by Hermes-compatible integrations.
//!
//! One request per line on the input stream, one response per line on the
//! output stream (stdout is reserved for the protocol; logs go to stderr).
//!
//! ```text
//! → {"id":1,"method":"search","params":{"query":"memory layers","limit":5,"mode":"hybrid"}}
//! ← {"id":1,"ok":true,"result":{"hits":[{"chunk_id":3,"text":"…","score":0.93,
//!     "confidence":"FACT","citation":{"file_path":"…","line_range":[4,9],…}}]}}
//! → {"id":2,"method":"index","params":{"path":".plaesy"}}
//! → {"id":3,"method":"stats"}   → {"id":4,"method":"metrics"}   → {"id":5,"method":"ping"}
//! ```
//!
//! Every hit carries its citation (EV-01); errors are returned as
//! `{"ok":false,"error":{"kind":…,"message":…}}` and never end the session.

use crate::memory::error::{MemoryError, Result};
use crate::memory::search::{
    normalize_tier, Citation, CitationBuilder, SearchEngine, SearchFilter, SearchMode, Trust, TIERS,
};
use crate::memory::storage::SearchResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// A protocol request.
#[derive(Debug, Deserialize)]
pub struct Request {
    /// Caller-chosen correlation id, echoed back verbatim.
    #[serde(default)]
    pub id: Value,

    /// Method name: `search`, `index`, `stats`, `metrics`, `ping`.
    pub method: String,

    /// Method parameters.
    #[serde(default)]
    pub params: Value,
}

/// A protocol response.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Response {
    /// The request's id (`null` if it could not be parsed).
    pub id: Value,

    /// Whether the call succeeded.
    pub ok: bool,

    /// Result payload on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    /// Error payload on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

/// Error payload.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ErrorBody {
    /// Machine-readable category.
    pub kind: String,

    /// Human-readable message.
    pub message: String,
}

/// A search hit with its mandatory citation.
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchHit {
    /// Chunk ID.
    pub chunk_id: u64,

    /// Verbatim chunk text.
    pub text: String,

    /// Relevance score (0-1).
    pub score: f32,

    /// `FACT`, `INFERRED` or `SPECULATIVE`.
    pub confidence: String,

    /// Source citation.
    pub citation: Citation,

    /// Optional leg scores breakdown for explain mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<crate::memory::storage::LegScores>,
}

impl SearchHit {
    /// Build a hit from a search result, validating its citation.
    pub fn from_result(r: &SearchResult) -> Result<Self> {
        let citation = CitationBuilder::new().build(r)?;
        Ok(SearchHit {
            chunk_id: r.chunk_id,
            text: r.text.clone(),
            score: r.score,
            confidence: citation.confidence.clone(),
            citation,
            explain: r.explain.clone(),
        })
    }
}

fn ok(id: Value, result: Value) -> Response {
    Response {
        id,
        ok: true,
        result: Some(result),
        error: None,
    }
}

fn err(id: Value, kind: &str, message: impl Into<String>) -> Response {
    Response {
        id,
        ok: false,
        result: None,
        error: Some(ErrorBody {
            kind: kind.to_string(),
            message: message.into(),
        }),
    }
}

fn error_kind(e: &MemoryError) -> &'static str {
    match e {
        MemoryError::ConfigError(_) => "config",
        MemoryError::FeatureNotEnabled { .. } => "feature_not_enabled",
        MemoryError::ResourceLimitExceeded(_) => "timeout",
        MemoryError::IoError { .. } => "io",
        _ => "internal",
    }
}

/// Handle one request against `engine`.
pub async fn handle_request(engine: &SearchEngine, req: Request) -> Response {
    let id = req.id.clone();
    match dispatch(engine, &req).await {
        Ok(v) => ok(id, v),
        Err(DispatchError::BadRequest(m)) => err(id, "bad_request", m),
        Err(DispatchError::Engine(e)) => err(id, error_kind(&e), e.to_string()),
    }
}

const MAX_REQUEST_LIMIT: u64 = 1_000;

fn request_limit(params: &Value, default: u64) -> std::result::Result<usize, DispatchError> {
    let raw = match params.get("limit") {
        None => default,
        Some(value) => value.as_u64().ok_or_else(|| {
            DispatchError::BadRequest("params.limit must be a non-negative integer".into())
        })?,
    };
    if raw == 0 || raw > MAX_REQUEST_LIMIT {
        return Err(DispatchError::BadRequest(format!(
            "params.limit must be between 1 and {MAX_REQUEST_LIMIT}"
        )));
    }
    usize::try_from(raw).map_err(|_| {
        DispatchError::BadRequest("params.limit is too large for this platform".into())
    })
}

enum DispatchError {
    BadRequest(String),
    Engine(MemoryError),
}

impl From<MemoryError> for DispatchError {
    fn from(e: MemoryError) -> Self {
        DispatchError::Engine(e)
    }
}

async fn dispatch(
    engine: &SearchEngine,
    req: &Request,
) -> std::result::Result<Value, DispatchError> {
    match req.method.as_str() {
        "ping" => Ok(json!({ "pong": true, "vector_search": engine.has_vector_search() })),
        "search" => {
            let query = req
                .params
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    DispatchError::BadRequest("params.query (string) is required".into())
                })?;
            let limit = request_limit(&req.params, 10)?;
            let mode_name = req
                .params
                .get("mode")
                .and_then(Value::as_str)
                .unwrap_or("hybrid");
            let mode = SearchMode::parse(mode_name).ok_or_else(|| {
                DispatchError::BadRequest(format!(
                    "unknown mode '{mode_name}' (expected hybrid, semantic or keyword)"
                ))
            })?;
            let filter = parse_filter(&req.params).map_err(DispatchError::BadRequest)?;
            let expand = req
                .params
                .get("expand_links")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let explain = req
                .params
                .get("explain")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let results = if expand {
                engine
                    .search_expanded_with_explain(query, limit, mode, &filter, explain)
                    .await?
            } else if explain {
                engine
                    .search_with_explain(query, limit, mode, &filter, true)
                    .await?
            } else {
                engine.search_with(query, limit, mode, &filter).await?
            };
            engine.note_query(query, &results)?;
            let hits: Vec<SearchHit> = results
                .iter()
                .map(SearchHit::from_result)
                .collect::<Result<_>>()?;
            Ok(json!({ "hits": hits }))
        }
        "missing" => {
            let limit = request_limit(&req.params, 20)?;
            let root = req
                .params
                .get("root")
                .and_then(Value::as_str)
                .map(PathBuf::from);
            let report = engine.missing(root, limit).await?;
            Ok(json!({ "query_gaps": report.query_gaps, "structural": report.structural }))
        }
        "resolve_gap" => {
            let id = req
                .params
                .get("id")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    DispatchError::BadRequest("params.id (integer) is required".into())
                })?;
            Ok(json!({ "resolved": engine.resolve_gap(id)? }))
        }
        "index" => {
            let path = req
                .params
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    DispatchError::BadRequest("params.path (string) is required".into())
                })?;
            let root = PathBuf::from(path);
            if !root.is_dir() {
                return Err(DispatchError::BadRequest(format!(
                    "{path} is not a directory"
                )));
            }
            let s = engine.index_directory(&root).await?;
            Ok(json!({
                "indexed": s.indexed, "unchanged": s.unchanged, "removed": s.removed,
                "chunks": s.chunks,
                "failed": s.failed.iter().map(|(p, e)| json!({"path": p, "error": e})).collect::<Vec<_>>(),
            }))
        }
        "backlinks" | "links" => {
            let path = req
                .params
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    DispatchError::BadRequest("params.path (string) is required".into())
                })?;
            let links = engine.links(path, req.method == "backlinks").await?;
            Ok(json!({ "links": links }))
        }
        "hygiene" => {
            let report = engine
                .hygiene(&crate::memory::hygiene::HygieneOptions {
                    root: req
                        .params
                        .get("root")
                        .and_then(Value::as_str)
                        .map(PathBuf::from),
                    ..Default::default()
                })
                .await?;
            Ok(serde_json::to_value(report)
                .map_err(|e| MemoryError::SerializationError(e.to_string()))?)
        }
        "stats" => {
            let s = engine.statistics().await?;
            Ok(serde_json::to_value(s)
                .map_err(|e| MemoryError::SerializationError(e.to_string()))?)
        }
        "metrics" => Ok(json!({ "prometheus": engine.metrics().render_prometheus() })),
        other => Err(DispatchError::BadRequest(format!(
            "unknown method '{other}'"
        ))),
    }
}

/// Build a [`SearchFilter`] from `params.tier` (string or array), `params.path_prefix`,
/// `params.min_trust` and `params.include_inactive`.
pub fn parse_filter(params: &Value) -> std::result::Result<SearchFilter, String> {
    let mut f = SearchFilter::default();
    match params.get("tier") {
        None | Some(Value::Null) => {}
        Some(Value::String(t)) => f.tiers.push(t.clone()),
        Some(Value::Array(a)) => {
            for t in a {
                f.tiers.push(
                    t.as_str()
                        .ok_or("params.tier must be a string or array of strings")?
                        .to_string(),
                );
            }
        }
        Some(_) => return Err("params.tier must be a string or array of strings".into()),
    }
    for t in &f.tiers {
        if normalize_tier(t).is_none() {
            return Err(format!(
                "unknown tier '{t}' (expected one of: {})",
                TIERS.join(", ")
            ));
        }
    }
    if let Some(v) = params.get("as_of").filter(|v| !v.is_null()) {
        let s = v
            .as_str()
            .ok_or("params.as_of must be a date string (YYYY-MM-DD)")?;
        f.as_of = Some(
            crate::memory::search::parse_date(s)
                .ok_or("params.as_of must be a date (YYYY-MM-DD) or an RFC 3339 timestamp")?,
        );
    }
    if let Some(p) = params.get("path_prefix").and_then(Value::as_str) {
        f.path_prefix = Some(p.to_string());
    }
    if let Some(v) = params.get("min_trust").filter(|v| !v.is_null()) {
        f.min_trust = Some(
            Trust::parse(v).ok_or("params.min_trust must be high, medium, low or a number 0-1")?,
        );
    }
    f.include_inactive = params
        .get("include_inactive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(f)
}

async fn write_line<W: AsyncWrite + Unpin>(out: &mut W, resp: &Response) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(resp).unwrap_or_else(|_| {
        br#"{"id":null,"ok":false,"error":{"kind":"internal","message":"response serialization failed"}}"#.to_vec()
    });
    line.push(b'\n');
    out.write_all(&line).await?;
    out.flush().await
}

/// Serve requests from `input` until EOF, writing responses to `output`.
///
/// Malformed lines produce an error response and the session continues.
///
/// # Errors
///
/// - `IoError` if reading or writing the streams fails
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
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle_request(engine, req).await,
            Err(e) => err(
                Value::Null,
                "bad_request",
                format!("invalid request JSON: {e}"),
            ),
        };
        write_line(&mut output, &resp).await.map_err(io_err)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "service_test.rs"]
mod service_test;

#[cfg(test)]
#[path = "service_filter_param_tests_test.rs"]
mod service_filter_param_tests_test;

/// Adversarial fuzzing for `request_limit`: arbitrary JSON values must either be accepted as
/// `1..=1000` or rejected as a `bad_request`, and must never panic.
#[cfg(test)]
#[path = "service_fuzz_test.rs"]
mod service_fuzz_test;
