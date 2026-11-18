//! Storage config types (split out of `storage/mod.rs`).

use super::*;

/// Database configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Path to SQLite database file (`:memory:` for an in-memory database).
    pub db_path: PathBuf,

    /// Enable WAL mode (write-ahead log) for crash recovery.
    pub wal_mode: bool,

    /// Cache size in pages (negative = KiB, positive = pages).
    pub cache_size: i32,

    /// Maximum number of concurrent readers.
    pub max_readers: usize,

    /// Enable FTS5 full-text search.
    pub enable_fts: bool,

    /// HNSW graph configuration.
    pub hnsw_config: HnswConfig,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig {
            db_path: PathBuf::from("memory.db"),
            wal_mode: true,
            cache_size: -64_000, // 64 MB
            max_readers: 10,
            enable_fts: true,
            hnsw_config: HnswConfig::default(),
        }
    }
}

/// HNSW (Hierarchical Navigable Small World) index configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Max neighbors per node (default 32, higher = more accurate).
    pub m: usize,

    /// Construction neighbors to explore (default 256).
    pub ef_construct: usize,

    /// Runtime neighbors to explore (default 128).
    pub ef_search: usize,

    /// Max neighbors for layer 0 (usually 2×m).
    pub max_m0: usize,

    /// Maximum number of layers.
    pub max_layers: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        HnswConfig {
            m: 32,
            ef_construct: 256,
            ef_search: 128,
            max_m0: 64,
            max_layers: 16,
        }
    }
}
