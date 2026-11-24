//! Storage subsystem for metadata, vectors, and indexes.
//!
//! Manages two persistent stores:
//! - **SQLite**: Documents, chunks, metadata, FTS5 full-text index (WAL mode)
//! - **HNSW Graph**: Vector index for semantic search (usearch, saved to disk)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────┐
//! │ SQLite Database (state.db)      │
//! ├─────────────────────────────────┤
//! │ Table: documents                │
//! │ Table: chunks                   │
//! │ Table: index_metadata           │
//! │ Virtual: chunks_fts (FTS5)      │
//! └─────────────────────────────────┘
//!
//! ┌─────────────────────────────────┐
//! │ HNSW Graph (usearch)            │
//! ├─────────────────────────────────┤
//! │ key: chunk_id → vector (cosine) │
//! └─────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use plaesy::memory::storage::{Database, DatabaseConfig};
//!
//! let db = Database::new(DatabaseConfig::default()).await?;
//! let doc_id = db.insert_document(&doc).await?;
//! db.insert_chunks(doc_id, &doc.chunks).await?;
//! let results = db.search_fts("keyword", 10).await?;
//! ```

use crate::memory::embeddings::EmbeddingVector;
use crate::memory::error::{MemoryError, Result};
use crate::memory::indexer::{Chunk, ChunkKind, Document};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod documents;
mod queue;
mod retrieval;
mod transaction;

mod config;
mod hnsw;
mod links;
mod types;

pub use config::{DatabaseConfig, HnswConfig};
pub use hnsw::HnswIndex;
pub use links::{extract_wikilinks, plausible_target, LinkRef};
pub use types::{ConfidenceLevel, DatabaseStats, HnswStats, LegScores, ResultMeta, SearchResult};

const SCHEMA_VERSION: &str = "2";

/// The model every database built before model recording was made with.
const LEGACY_EMBEDDING_MODEL: &str = "all-MiniLM-L6-v2";

/// BM25 value at which the normalised keyword score reaches 0.5.
/// A lone weak term match scores ~0.2; strong multi-term matches 0.5+.
const BM25_HALF_SATURATION: f64 = 8.0;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS documents (
    id          INTEGER PRIMARY KEY,
    source_path TEXT NOT NULL UNIQUE,
    hash        TEXT,
    revision    TEXT,
    metadata    TEXT NOT NULL DEFAULT '{}',
    body        TEXT NOT NULL,
    indexed_at  TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chunks (
    id          INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    text        TEXT NOT NULL,
    kind        TEXT NOT NULL,
    layer       INTEGER NOT NULL,
    start_line  INTEGER NOT NULL,
    end_line    INTEGER NOT NULL,
    metadata    TEXT,
    embedding   BLOB
);
CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);
CREATE TABLE IF NOT EXISTS links (
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    target      TEXT NOT NULL,
    line        INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_links_document ON links(document_id);
CREATE TABLE IF NOT EXISTS index_metadata (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS pending_index (
    source_path TEXT PRIMARY KEY,
    queued_at   TEXT NOT NULL
);
";

const FTS_SCHEMA: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    text, content='chunks', content_rowid='id'
);
CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE OF text ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
    INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
";

fn db_err(e: rusqlite::Error) -> MemoryError {
    MemoryError::DatabaseError(e.to_string())
}

/// Increment the persistent change counter used to validate HNSW snapshots.
/// Called by every write that adds, replaces or removes chunks or embeddings.
fn bump_generation(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO index_metadata(key, value) VALUES ('generation', '1')
         ON CONFLICT(key) DO UPDATE SET value = CAST(value AS INTEGER) + 1",
        [],
    )?;
    Ok(())
}

fn json_err(e: serde_json::Error) -> MemoryError {
    MemoryError::SerializationError(e.to_string())
}

fn kind_to_str(kind: ChunkKind) -> &'static str {
    match kind {
        ChunkKind::Summary => "summary",
        ChunkKind::Detail => "detail",
        ChunkKind::Code => "code",
        ChunkKind::Quote => "quote",
        ChunkKind::Heading => "heading",
    }
}

fn kind_from_str(s: &str) -> ChunkKind {
    match s {
        "summary" => ChunkKind::Summary,
        "code" => ChunkKind::Code,
        "quote" => ChunkKind::Quote,
        "heading" => ChunkKind::Heading,
        _ => ChunkKind::Detail,
    }
}

fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

/// Convert a free-text query into a safe FTS5 expression.
///
/// Each alphanumeric token is double-quoted (so FTS5 operators in user input
/// are inert) and tokens are OR-ed. Returns `None` if no tokens remain.
///
/// Common English stopwords are dropped so they don't match every chunk; if
/// the query consists only of stopwords they are kept rather than returning nothing.
pub fn fts_query(query: &str) -> Option<String> {
    const STOPWORDS: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "from", "has", "have", "in",
        "is", "it", "its", "of", "on", "or", "that", "the", "this", "to", "was", "were", "will",
        "with",
    ];
    let all: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .collect();
    let content: Vec<&str> = all
        .iter()
        .copied()
        .filter(|t| !STOPWORDS.contains(&t.to_lowercase().as_str()))
        .collect();
    let chosen = if content.is_empty() { all } else { content };
    if chosen.is_empty() {
        None
    } else {
        Some(
            chosen
                .iter()
                .map(|t| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(" OR "),
        )
    }
}

const RESULT_SELECT: &str = "SELECT c.id, c.text, d.source_path, c.start_line, c.end_line, \
                             d.revision, d.indexed_at FROM chunks c JOIN documents d ON d.id = c.document_id";

fn row_to_result(
    row: &rusqlite::Row<'_>,
    score: f32,
    confidence: ConfidenceLevel,
) -> rusqlite::Result<SearchResult> {
    Ok(SearchResult {
        chunk_id: row.get::<_, i64>(0)? as u64,
        text: row.get(1)?,
        source_path: row.get(2)?,
        start_line: row.get::<_, i64>(3)? as usize,
        end_line: row.get::<_, i64>(4)? as usize,
        revision: row.get(5)?,
        indexed_at: row.get(6)?,
        score,
        confidence,
        meta: ResultMeta::default(),
        explain: None,
    })
}

/// A metadata-only entry returned by indexed-note listing.
#[derive(Clone, Debug)]
pub struct ListedDocument {
    /// Path relative to the indexed root.
    pub path: String,
    /// YAML frontmatter.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// A document as stored in the index.
#[derive(Clone, Debug)]
pub struct StoredDocument {
    /// Path relative to the indexed root.
    pub path: String,

    /// YAML frontmatter.
    pub metadata: HashMap<String, serde_json::Value>,

    /// Markdown body (frontmatter removed).
    pub body: String,

    /// 1-based line of the source file where `body` begins.
    pub body_start_line: usize,
}

#[cfg(test)]
#[path = "storage_test.rs"]
mod storage_test;

/// Resolve a wikilink target against known document paths.
///
/// Matches (case-insensitively) the full path, the path without extension, or the file
/// stem. Ambiguous stems resolve to the lexicographically first path so results are stable.
pub fn resolve_link(target: &str, paths: &[String]) -> Option<String> {
    let t = target.trim().trim_end_matches(".md").to_lowercase();
    let t = t.replace('\\', "/");
    let mut hits: Vec<&String> = paths
        .iter()
        .filter(|p| {
            let low = p.to_lowercase();
            let no_ext = low.rsplit_once('.').map(|(a, _)| a).unwrap_or(&low);
            let stem = no_ext.rsplit('/').next().unwrap_or(no_ext);
            low == t
                || no_ext == t
                || no_ext.ends_with(&format!("/{t}"))
                || (!t.contains('/') && stem == t)
        })
        .collect();
    hits.sort();
    hits.first().map(|s| (*s).clone())
}

/// SQLite-backed document storage.
pub struct Database {
    config: DatabaseConfig,
    conn: Mutex<Connection>,
}

impl Database {
    /// Create a new database instance.
    ///
    /// Initializes SQLite schema (documents, chunks, metadata tables) on first open.
    ///
    /// # Errors
    ///
    /// - `DatabaseError` if cannot open or initialize database
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        Self::open(config)
    }

    fn open(config: DatabaseConfig) -> Result<Self> {
        let in_memory = config.db_path.as_os_str() == ":memory:";
        if !in_memory {
            if let Some(parent) = config.db_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| MemoryError::IoError {
                        path: parent.display().to_string(),
                        source: e,
                    })?;
                }
            }
        }
        let conn = Connection::open(&config.db_path).map_err(db_err)?;
        // The CLI, the MCP server and a scheduled job may write at the same time; wait for the
        // lock instead of failing with "database is locked".
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_err)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(db_err)?;
        if config.wal_mode && !in_memory {
            conn.query_row("PRAGMA journal_mode = WAL;", [], |_| Ok(()))
                .map_err(db_err)?;
            // WAL is the intended deployment mode: readers can continue while the single
            // application writer commits. NORMAL preserves WAL durability while avoiding an
            // fsync for every frame; the writer queue and short transactions bound contention.
            conn.execute_batch("PRAGMA synchronous = NORMAL; PRAGMA wal_autocheckpoint = 1000;")
                .map_err(db_err)?;
        }
        conn.execute_batch(&format!("PRAGMA cache_size = {};", config.cache_size))
            .map_err(db_err)?;
        conn.execute_batch(SCHEMA).map_err(db_err)?;
        if config.enable_fts {
            conn.execute_batch(FTS_SCHEMA).map_err(db_err)?;
        }
        conn.execute(
            "INSERT OR IGNORE INTO index_metadata(key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION],
        )
        .map_err(db_err)?;
        let has_body_start: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('documents') WHERE name = 'body_start'")
            .and_then(|mut st| st.exists([]))
            .map_err(db_err)?;
        if !has_body_start {
            conn.execute_batch(
                "ALTER TABLE documents ADD COLUMN body_start INTEGER NOT NULL DEFAULT 1;",
            )
            .map_err(db_err)?;
        }
        // v1 -> v2 added the `links` table. Documents indexed under v1 have no link rows, so
        // clear their hashes: the next sync sees them as changed and re-indexes them.
        let stored: String = conn
            .query_row(
                "SELECT value FROM index_metadata WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        if stored != SCHEMA_VERSION {
            conn.execute_batch("UPDATE documents SET hash = NULL;")
                .map_err(db_err)?;
            conn.execute(
                "UPDATE index_metadata SET value = ?1 WHERE key = 'schema_version'",
                params![SCHEMA_VERSION],
            )
            .map_err(db_err)?;
        }
        Ok(Database {
            config,
            conn: Mutex::new(conn),
        })
    }

    /// Record which embedding model the stored vectors come from.
    ///
    /// A database with documents but no record was built with the original model
    /// (`all-MiniLM-L6-v2`). If a different model was recorded, or the model changed now, every
    /// document's hash is cleared so the next sync re-embeds it. Returns `true` on such a change.
    pub async fn record_embedding_model(&self, model: &str) -> Result<bool> {
        self.record_embedding_identity(model).await
    }

    /// Record the exact embedding runtime identity used by stored vectors.
    pub async fn record_embedding_identity(&self, identity: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let recorded: Option<String> = conn
            .query_row(
                "SELECT value FROM index_metadata WHERE key = 'embedding_model'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        let previous = match recorded {
            Some(m) => Some(m),
            None => {
                let docs: i64 = conn
                    .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
                    .map_err(db_err)?;
                (docs > 0).then(|| LEGACY_EMBEDDING_MODEL.to_string())
            }
        };
        let changed = previous.as_deref().is_some_and(|p| p != identity);
        if changed {
            conn.execute_batch("UPDATE documents SET hash = NULL;")
                .map_err(db_err)?;
        }
        conn.execute(
            "INSERT INTO index_metadata(key, value) VALUES ('embedding_model', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![identity],
        )
        .map_err(db_err)?;
        Ok(changed)
    }

    /// Persistent change counter: increases on every write that can affect the vector index.
    pub async fn generation(&self) -> Result<u64> {
        let conn = self.conn.lock();
        let v: Option<String> = conn
            .query_row(
                "SELECT value FROM index_metadata WHERE key = 'generation'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    /// HNSW parameters from the database configuration.
    pub fn hnsw_config(&self) -> &HnswConfig {
        &self.config.hnsw_config
    }

    /// Lock the SQLite connection. Crate-internal so feature tables (see `gaps`) can sit beside the core schema.
    pub(crate) fn conn(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.conn.lock()
    }

    /// Path of the database file, or `None` for an in-memory database.
    pub fn path(&self) -> Option<&Path> {
        let p = self.config.db_path.as_path();
        (p.as_os_str() != ":memory:").then_some(p)
    }
}

#[cfg(test)]
#[path = "storage_tests_test.rs"]
mod storage_tests_test;
