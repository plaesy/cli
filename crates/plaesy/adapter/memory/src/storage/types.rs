//! Storage types types (split out of `storage/mod.rs`).

use super::*;

/// Result of a search query (single match) with full citation data (EV-01).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// Chunk ID.
    pub chunk_id: u64,

    /// Chunk text (verbatim stored text, never generated).
    pub text: String,

    /// Source file path.
    pub source_path: String,

    /// First line of the chunk in the source document.
    pub start_line: usize,

    /// Last line of the chunk in the source document.
    pub end_line: usize,

    /// Git revision of the source at index time, if known.
    pub revision: Option<String>,

    /// RFC 3339 timestamp when the source was indexed.
    pub indexed_at: String,

    /// Relevance score (0-1).
    pub score: f32,

    /// Confidence level (FACT, INFERRED, SPECULATIVE).
    pub confidence: ConfidenceLevel,

    /// Source-document metadata (tier, trust, freshness). Filled by the search engine.
    #[serde(default)]
    pub meta: ResultMeta,

    /// Per-leg score breakdown, populated only when explain is requested.
    #[serde(default)]
    pub explain: Option<LegScores>,
}

/// Document-level metadata attached to a search result.
///
/// Read from the source file's YAML frontmatter (`tier`, `confidence`, `last_verified`,
/// `status`, `supersedes`); `tier` falls back to the path (see `search::infer_tier`).
/// This is the *source's* declared trust, distinct from the per-result FACT/INFERRED label.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResultMeta {
    /// Memory tier: `core`, `episodic`, `semantic`, `procedural` or `archive`.
    pub tier: Option<String>,

    /// Declared trust of the source: `high`, `medium` or `low`.
    pub trust: Option<String>,

    /// Date the source's claims were last verified (as written in the file).
    pub last_verified: Option<String>,

    /// Lifecycle status, e.g. `active`, `superseded`, `deprecated`.
    pub status: Option<String>,

    /// Tags explicitly declared by the source.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Project that owns the source, when declared.
    pub project: Option<String>,

    /// Scopes this source belongs to, when declared.
    #[serde(default)]
    pub scope: Vec<String>,

    /// Explicit event date declared in source frontmatter.
    pub event_date: Option<String>,

    /// First day the source's claims hold (`valid_from` in frontmatter), if declared.
    pub valid_from: Option<String>,

    /// Last day the source's claims hold (`valid_to`, inclusive), if declared.
    pub valid_to: Option<String>,

    /// `true` when `last_verified` is older than the configured staleness window.
    pub stale: bool,
}

/// Per-leg score breakdown attached to a search result for `--explain`.
///
/// `None` means that leg did not contribute to this result. Each field is an
/// `Option<f32>` so keyword-only results can omit the vector leg and vice versa.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LegScores {
    /// Raw full-text (BM25) score, if present.
    #[serde(default)]
    pub fts: Option<f32>,
    /// Raw embedding (cosine distance → similarity) score, if present.
    #[serde(default)]
    pub vector: Option<f32>,
    /// Fused score after RRF / weighted fusion, before re-ranking.
    #[serde(default)]
    pub fused: Option<f32>,
    /// Staleness factor: 1.0 when not stale, reduced when the source is stale.
    #[serde(default)]
    pub staleness_factor: Option<f32>,
    /// Re-ranked score, if the reranker was enabled and applied.
    #[serde(default)]
    pub rerank: Option<f32>,
}

/// Confidence level for a search result.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConfidenceLevel {
    /// Directly from source (no inference).
    Fact,

    /// Inferred from context.
    Inferred,

    /// Speculative (model-added content).
    Speculative,
}

/// Database statistics (diagnostic).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseStats {
    /// Total number of documents.
    pub documents_total: u64,

    /// Total number of chunks.
    pub chunks_total: u64,

    /// Database file size in bytes.
    pub db_size_bytes: u64,

    /// FTS index size in bytes.
    pub fts_index_size_bytes: u64,

    /// HNSW graph size in bytes (filled by the owner of the index).
    pub hnsw_size_bytes: u64,

    /// Coverage percentage (indexed files / total files in .plaesy/).
    pub coverage_percent: f64,
}

/// HNSW graph statistics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswStats {
    /// Total number of vectors in the graph.
    pub nodes_total: u64,

    /// Vector dimension.
    pub dimensions: usize,

    /// Configured connectivity (max neighbors per node).
    pub connectivity: usize,

    /// Approximate memory used by the index in bytes.
    pub memory_bytes: u64,
}
