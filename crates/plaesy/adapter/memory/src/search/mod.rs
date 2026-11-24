//! Search subsystem for query execution and ranking.
//!
//! Combines vector search, keyword search, and re-ranking:
//! - **Vector Search**: HNSW semantic similarity
//! - **Keyword Search**: FTS5 BM25 ranking
//! - **Hybrid Ranking**: Reciprocal Rank Fusion (RRF)
//! - **Citations**: Build structured citations for results (EV-01 compliance)
//! - **Indexing**: `SearchEngine` also owns the write path (extract → chunk →
//!   embed → store) so the SQLite and HNSW stores never diverge
//!
//! # Architecture
//!
//! ```text
//! Query: "memory architecture"
//!     │
//!     ├─▶ Embed query (ONNX) ──▶ Vector Search (HNSW)  ─┐
//!     │                                                  ├─▶ RRF merge ─▶ Top-N + citations
//!     └─▶ Tokenize query ──────▶ Keyword Search (FTS5) ─┘
//! ```
//!
//! If no embedding model is available the engine runs keyword-only; if the
//! vector leg fails at query time the keyword results are still returned.
//!
//! # Evidence labels
//!
//! Result text is always verbatim stored text (EV-03). Confidence is `Fact`
//! when the result matched lexically (FTS5), and `Inferred` when it was found
//! by embedding similarity alone (EV-02 / NN-03).
//!
//! # Example
//!
//! ```ignore
//! use plaesy::memory::config::MemoryConfig;
//! use plaesy::memory::search::{CitationBuilder, SearchEngine};
//!
//! let engine = SearchEngine::from_config(&MemoryConfig::default()).await?;
//! engine.index_directory(".plaesy".as_ref()).await?;
//! for r in engine.search("memory architecture", 10).await? {
//!     println!("{}", CitationBuilder::new().build(&r)?);
//! }
//! ```

mod filter;
pub use filter::{
    age_days, build_meta, infer_tier, normalize_tier, parse_date, SearchFilter, Trust, TIERS,
};

pub mod rerank;

mod citation;
mod ranking;
mod support;

pub use citation::{Citation, CitationBuilder};
pub use ranking::{Ranker, RankerConfig, RankerType, SearchMode};
pub use support::fingerprint_note_in_root;
use support::{archived_note_content, git_revision, reconcile_stale_temp_files};
// Used by the test modules via `super::*`; not needed by non-test code.
#[cfg(test)]
use citation::docid_for;

use crate::memory::config::{MemoryConfig, SearchConfig};
use crate::memory::embeddings::{EmbeddingEngine, EmbeddingVector};
use crate::memory::error::{MemoryError, Result};
use crate::memory::indexer::{
    is_excluded, is_indexable, is_skipped_dir_name, scan_dir, Chunker, Document, EventKind,
    Extractor, FileEvent,
};
use crate::memory::metrics::{IndexHealth, Metrics, SearchType, Timer};
use crate::memory::notes::WriteLock;
use crate::memory::search::rerank::{sigmoid, CrossEncoder};
use crate::memory::storage::{
    ConfidenceLevel, Database, DatabaseStats, HnswIndex, ResultMeta, SearchResult,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

mod indexing;
mod query;
mod snapshot;

/// RRF damping constant (Cormack et al. 2009 use 60).
const RRF_K: f32 = 60.0;

/// Result of indexing one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexOutcome {
    /// Document stored. `embedded` is false when no vectors were produced
    /// (no embedding engine, or embedding failed) — it is still keyword-searchable.
    Indexed {
        /// Number of chunks stored.
        chunks: usize,
        /// Whether vectors were stored for the chunks.
        embedded: bool,
    },

    /// Content hash matched the stored document; nothing was written.
    Unchanged,

    /// A vault tombstone suppresses this note and removed its indexed copy.
    Forgotten,
}

enum NoteLifecycleOperation {
    Update { reason: String },
    Archive { reason: String, from_state: String },
}

enum NoteLifecycleIntent {
    Update(crate::memory::lifecycle::UpdateIntent),
    Archive(crate::memory::lifecycle::ArchiveIntent),
}

/// Summary of a directory sync.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexSummary {
    /// Files (re)indexed.
    pub indexed: usize,
    /// Files skipped because their content hash was unchanged.
    pub unchanged: usize,
    /// Stored documents removed because the file no longer exists.
    pub removed: usize,
    /// Chunks written during this sync.
    pub chunks: usize,
    /// Files that failed: `(path, error message)`. Failures never abort the sync.
    pub failed: Vec<(String, String)>,
    /// Files with no text to index (empty or whitespace only). Skipped, not a failure.
    pub skipped_empty: usize,
}

/// Top hits whose wikilinks are followed by `search_expanded`.
const LINK_SEED_LIMIT: usize = 3;
/// Linked notes examined per `search_expanded` call.
const LINK_NEIGHBOUR_LIMIT: usize = 5;
/// Candidates fetched per linked note (the note's own passages are few).
const LINK_NEIGHBOUR_SEARCH: usize = 20;
/// Score multiplier for passages reached through a link rather than matched directly.
const LINK_SCORE_FACTOR: f32 = 0.8;

/// Search engine orchestrating vector + keyword search, ranking, and indexing.
pub struct SearchEngine {
    db: Arc<Database>,
    embeddings: Option<Arc<EmbeddingEngine>>,
    hnsw: RwLock<HnswIndex>,
    ranker: Ranker,
    reranker: Option<Arc<CrossEncoder>>,
    extractor: Extractor,
    chunker: Chunker,
    config: SearchConfig,
    metrics: Arc<Metrics>,
    /// Serialises writers within this process; WriteLock also serialises every mutator across processes.
    write_lock: tokio::sync::Mutex<()>,
    /// Database generation captured by the last HNSW snapshot (`u64::MAX` = none).
    snapshot_generation: AtomicU64,
    /// False while stored documents need re-embedding or vector search is disabled.
    vector_index_ready: AtomicBool,
    /// Folder the index mirrors; set once at startup so `statistics` can report coverage.
    watch_root: std::sync::OnceLock<PathBuf>,
    /// True when this engine is owned by the long-running daemon.
    daemon_mode: std::sync::atomic::AtomicBool,
    /// Mode baca-saja (`serve --read-only`): tool penulisan MCP ditolak.
    read_only: std::sync::atomic::AtomicBool,
}

impl SearchEngine {
    /// Create a search engine over an open database.
    ///
    /// The HNSW index is loaded or rebuilt only when the semantic index is current.
    /// `embeddings = None` gives keyword-only operation without loading the vector index.
    ///
    /// # Errors
    ///
    /// - `HnswIndexCorruption` / `DimensionMismatch` if the index cannot be built from stored vectors
    pub async fn new(
        db: Arc<Database>,
        embeddings: Option<Arc<EmbeddingEngine>>,
        config: SearchConfig,
    ) -> Result<Self> {
        let dim = embeddings.as_ref().map(|e| e.dimension()).unwrap_or(384);
        // Vectors from a different model are not comparable: when the model changed, the stored
        // documents are queued for re-embedding and the HNSW snapshot (built from the old model)
        // is not reused.
        let model_changed = match &embeddings {
            Some(e) => {
                let identity = e.identity()?;
                let changed = db.record_embedding_identity(&identity).await?;
                if changed {
                    tracing::warn!(
                        model = e.model_name(),
                        "embedding runtime changed; documents are re-embedded on the next sync"
                    );
                }
                changed
            }
            None => false,
        };
        let needs_reindex = embeddings.is_some() && db.has_unindexed_documents().await?;
        let vector_index_ready =
            config.enable_vector_search && embeddings.is_some() && !needs_reindex;
        let generation = db.generation().await?;
        let snapshot = if !vector_index_ready || model_changed {
            None
        } else {
            Self::load_snapshot(&db, dim, generation).await
        };
        let (hnsw, snapshot_generation) = if !vector_index_ready {
            (
                HnswIndex::with_dimensions(db.hnsw_config().clone(), dim)?,
                u64::MAX,
            )
        } else {
            match snapshot {
                Some(h) => (h, generation),
                None => {
                    tracing::warn!(
                        "semantic index snapshot is missing or stale; rebuilding from stored embeddings"
                    );
                    let mut h = HnswIndex::with_dimensions(db.hnsw_config().clone(), dim)?;
                    for (id, v) in db.all_embeddings().await? {
                        h.insert(id, &v)?;
                    }
                    (h, u64::MAX)
                }
            }
        };
        Ok(SearchEngine {
            db,
            embeddings,
            hnsw: RwLock::new(hnsw),
            ranker: Ranker::new(RankerConfig::default()),
            reranker: None,
            extractor: Extractor::new(),
            chunker: Chunker::new(),
            config,
            metrics: Arc::new(Metrics::new()),
            write_lock: tokio::sync::Mutex::new(()),
            snapshot_generation: AtomicU64::new(snapshot_generation),
            vector_index_ready: AtomicBool::new(vector_index_ready),
            watch_root: std::sync::OnceLock::new(),
            daemon_mode: std::sync::atomic::AtomicBool::new(false),
            read_only: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Build a search engine from configuration: opens the database and, if
    /// vector search is enabled, tries to load the embedding model. A missing
    /// model downgrades to keyword-only (logged as a warning), never an error.
    pub async fn from_config(config: &MemoryConfig) -> Result<Self> {
        config.validate()?;
        let mut db_cfg = config.database.clone();
        db_cfg.hnsw_config = config.database.hnsw_config.clone();
        let db = Arc::new(Database::new(db_cfg).await?);
        let embeddings = if config.search.enable_vector_search {
            match EmbeddingEngine::with_config(config.embeddings.clone()).await {
                Ok(e) => Some(Arc::new(e)),
                Err(e) => {
                    tracing::warn!("embeddings unavailable, running keyword-only: {e}");
                    None
                }
            }
        } else {
            None
        };
        let mut engine = Self::new(db, embeddings, config.search.clone()).await?;
        engine.set_watch_root(config.watch_dir.clone());
        if engine.has_vector_search() {
            // Persist a cold rebuild so ordinary read-only commands do not rebuild HNSW
            // again on every process start.
            engine.save_snapshot().await;
        } else if config.search.enable_vector_search && engine.embeddings.is_some() {
            tracing::warn!(
                "semantic documents need an index sync; run `plaesy memory index` before semantic search"
            );
        }
        if config.search.rerank {
            let model = CrossEncoder::resolve_model_dir(config.search.rerank_model_dir.as_deref())
                .ok_or_else(|| "no cross-encoder model found".to_string())
                .and_then(|dir| CrossEncoder::load(&dir, 256).map_err(|e| e.to_string()));
            match model {
                Ok(m) => engine.reranker = Some(Arc::new(m)),
                Err(e) => {
                    tracing::warn!("reranking requested but unavailable, continuing without: {e}");
                }
            }
        }
        Ok(engine)
    }

    /// Attach a cross-encoder; hybrid searches then rerank their top candidates.
    pub fn with_reranker(mut self, reranker: Arc<CrossEncoder>) -> Self {
        self.reranker = Some(reranker);
        self
    }

    /// Whether hybrid search reranks with a cross-encoder.
    pub fn has_reranker(&self) -> bool {
        self.config.rerank && self.reranker.is_some()
    }

    /// Rescore the first `top_n` of `results` with the cross-encoder and re-sort them.
    ///
    /// Final score = `(1 - weight) * fused + weight * sigmoid(logit)`. Results beyond `top_n`
    /// keep their order after the reranked block. Text stays verbatim (EV-03); the evidence
    /// label is unchanged because reranking does not change *why* a passage matched.
    ///
    /// # Errors
    ///
    /// - `FeatureNotEnabled` if no cross-encoder is attached
    /// - `EmbeddingInferenceError` if scoring fails
    pub async fn rerank_results(
        &self,
        query: &str,
        mut results: Vec<SearchResult>,
        top_n: usize,
        weight: f32,
    ) -> Result<Vec<SearchResult>> {
        let ce = self
            .reranker
            .as_ref()
            .ok_or(MemoryError::FeatureNotEnabled {
                feature: "reranking".to_string(),
            })?;
        let n = top_n.min(results.len());
        if n < 2 {
            return Ok(results);
        }
        let w = weight.clamp(0.0, 1.0);
        let docs: Vec<String> = results[..n].iter().map(|r| r.text.clone()).collect();
        let logits = ce.score(query, docs).await?;
        for (r, l) in results[..n].iter_mut().zip(logits) {
            r.score = (1.0 - w) * r.score + w * sigmoid(l);
        }
        results[..n].sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.chunk_id.cmp(&b.chunk_id))
        });
        Ok(results)
    }

    /// Metrics collector for this engine.
    pub fn metrics(&self) -> Arc<Metrics> {
        Arc::clone(&self.metrics)
    }

    /// Whether the semantic leg is available.
    pub fn has_vector_search(&self) -> bool {
        self.config.enable_vector_search
            && self.embeddings.is_some()
            && self.vector_index_ready.load(Ordering::Acquire)
    }

    /// Search settings in effect.
    pub fn search_config(&self) -> &SearchConfig {
        &self.config
    }

    /// Shared handle to the underlying database.
    pub fn database(&self) -> Arc<Database> {
        Arc::clone(&self.db)
    }

    /// Mark this engine as owned by a long-running daemon that already holds the
    /// cross-process writer lock for the vault lifetime.
    pub(crate) fn set_daemon_mode(&self, enabled: bool) {
        self.daemon_mode
            .store(enabled, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn daemon_mode(&self) -> bool {
        self.daemon_mode.load(std::sync::atomic::Ordering::Acquire)
    }

    pub(crate) fn set_read_only(&self, enabled: bool) {
        self.read_only
            .store(enabled, std::sync::atomic::Ordering::Release);
    }

    pub(crate) fn is_read_only(&self) -> bool {
        self.read_only.load(std::sync::atomic::Ordering::Acquire)
    }

    // -- Indexing ----------------------------------------------------------

    /// The notes root set by [`SearchEngine::set_watch_root`], if any.
    pub fn watch_root(&self) -> Option<&Path> {
        self.watch_root.get().map(PathBuf::as_path)
    }

    /// Record the folder the index mirrors. Call once at startup; later calls are ignored.
    pub fn set_watch_root(&self, root: PathBuf) {
        let _ = self.watch_root.set(root);
    }

    /// Database statistics including the HNSW memory footprint and, once a watch root is set,
    /// the share of files under it that are indexed.
    pub async fn statistics(&self) -> Result<DatabaseStats> {
        let mut s = self.db.statistics().await?;
        s.hnsw_size_bytes = self.hnsw.read().statistics().memory_bytes;
        if let Some(root) = self.watch_root.get() {
            s.coverage_percent = self.coverage(root).await?;
        }
        Ok(s)
    }

    /// Percentage of indexable files under `root` that are stored in the index (100 when none exist).
    async fn coverage(&self, root: &Path) -> Result<f64> {
        let on_disk: HashSet<String> = scan_dir(root)
            .iter()
            .filter(|f| !is_excluded(root, f, &self.config.exclude_dirs))
            .map(|f| {
                f.strip_prefix(root)
                    .unwrap_or(f)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        let stored: HashSet<String> = self
            .db
            .document_paths()
            .await?
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        Ok(if on_disk.is_empty() {
            100.0
        } else {
            on_disk.intersection(&stored).count() as f64 / on_disk.len() as f64 * 100.0
        })
    }

    /// Compute index health (coverage against files under `root`) and publish it to metrics.
    pub async fn update_health(&self, root: &Path) -> Result<IndexHealth> {
        let stats = self.statistics().await?;
        let coverage = self.coverage(root).await?;
        let health = IndexHealth {
            documents_total: stats.documents_total,
            chunks_total: stats.chunks_total,
            index_size_bytes: stats.db_size_bytes + stats.hnsw_size_bytes,
            coverage_percent: coverage,
            last_indexed: chrono::Utc::now().to_rfc3339(),
            is_corrupted: false,
        };
        self.metrics.update_index_health(&health);
        Ok(health)
    }

    // -- Searching ---------------------------------------------------------
}

#[cfg(test)]
#[path = "search_test.rs"]
mod search_test;

#[cfg(test)]
#[path = "search_snapshot_tests_test.rs"]
mod search_snapshot_tests_test;
