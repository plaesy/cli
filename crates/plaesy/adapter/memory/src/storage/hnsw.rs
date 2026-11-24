//! Storage hnsw types (split out of `storage/mod.rs`).

use super::*;

/// HNSW vector index for semantic search (cosine metric), backed by usearch.
pub struct HnswIndex {
    config: HnswConfig,
    dimensions: usize,
    index: usearch::Index,
}

/// Default embedding dimension (all-MiniLM-L6-v2).
const DEFAULT_DIMENSIONS: usize = 384;

fn hnsw_err(e: impl std::fmt::Display) -> MemoryError {
    MemoryError::HnswIndexCorruption(e.to_string())
}

fn build_usearch(config: &HnswConfig, dimensions: usize) -> Result<usearch::Index> {
    let options = usearch::IndexOptions {
        dimensions,
        metric: usearch::MetricKind::Cos,
        quantization: usearch::ScalarKind::F32,
        connectivity: config.m,
        expansion_add: config.ef_construct,
        expansion_search: config.ef_search,
        multi: false,
    };
    usearch::Index::new(&options).map_err(hnsw_err)
}

impl HnswIndex {
    /// Create a new HNSW index for vectors of `dimensions` length.
    ///
    /// # Errors
    ///
    /// - `HnswIndexCorruption` if the underlying index cannot be created
    pub fn with_dimensions(config: HnswConfig, dimensions: usize) -> Result<Self> {
        let index = build_usearch(&config, dimensions)?;
        Ok(HnswIndex {
            config,
            dimensions,
            index,
        })
    }

    /// Create a new HNSW index with the default 384 dimensions.
    ///
    /// # Errors
    ///
    /// - `HnswIndexCorruption` if the underlying index cannot be created
    pub fn new(config: HnswConfig) -> Result<Self> {
        Self::with_dimensions(config, DEFAULT_DIMENSIONS)
    }

    /// Vector dimension of this index.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Number of vectors stored.
    pub fn len(&self) -> usize {
        self.index.size()
    }

    /// Whether the index holds no vectors.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn ensure_capacity(&self, extra: usize) -> Result<()> {
        let need = self.index.size() + extra;
        if need > self.index.capacity() {
            self.index.reserve((need * 2).max(1024)).map_err(hnsw_err)?;
        }
        Ok(())
    }

    /// Add (or replace) a vector in the index.
    ///
    /// # Errors
    ///
    /// - `DimensionMismatch` if the vector length differs from the index dimension
    /// - `HnswIndexCorruption` if index state is invalid
    pub fn insert(&mut self, id: u64, vector: &EmbeddingVector) -> Result<()> {
        if vector.len() != self.dimensions {
            return Err(MemoryError::DimensionMismatch {
                expected: self.dimensions,
                got: vector.len(),
            });
        }
        if self.index.contains(id) {
            self.index.remove(id).map_err(hnsw_err)?;
        }
        self.ensure_capacity(1)?;
        self.index.add(id, vector).map_err(hnsw_err)
    }

    /// Remove a vector; returns whether it was present.
    pub fn remove(&mut self, id: u64) -> Result<bool> {
        if !self.index.contains(id) {
            return Ok(false);
        }
        self.index.remove(id).map_err(hnsw_err)?;
        Ok(true)
    }

    /// Search for nearest neighbors.
    ///
    /// Returns up to `k` `(id, cosine distance)` pairs, nearest first.
    pub fn search(&self, query: &EmbeddingVector, k: usize) -> Result<Vec<(u64, f32)>> {
        if query.len() != self.dimensions {
            return Err(MemoryError::DimensionMismatch {
                expected: self.dimensions,
                got: query.len(),
            });
        }
        if self.is_empty() || k == 0 {
            return Ok(Vec::new());
        }
        let m = self
            .index
            .search(query, k)
            .map_err(|e| MemoryError::VectorSearchError(e.to_string()))?;
        Ok(m.keys.into_iter().zip(m.distances).collect())
    }

    fn meta_path(path: &Path) -> PathBuf {
        let mut s = path.as_os_str().to_owned();
        s.push(".meta.json");
        PathBuf::from(s)
    }

    /// Load index from disk (the file written by [`HnswIndex::save`] plus its `.meta.json`).
    pub fn load(path: &Path) -> Result<Self> {
        let meta_path = Self::meta_path(path);
        let raw = std::fs::read_to_string(&meta_path).map_err(|e| MemoryError::IoError {
            path: meta_path.display().to_string(),
            source: e,
        })?;
        let (config, dimensions): (HnswConfig, usize) = serde_json::from_str(&raw)
            .map_err(|e| MemoryError::DeserializationError(e.to_string()))?;
        let index = build_usearch(&config, dimensions)?;
        index.load(&path.to_string_lossy()).map_err(hnsw_err)?;
        Ok(HnswIndex {
            config,
            dimensions,
            index,
        })
    }

    /// Save index to disk, plus a `<path>.meta.json` sidecar with config and dimensions.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| MemoryError::IoError {
                    path: parent.display().to_string(),
                    source: e,
                })?;
            }
        }
        self.index.save(&path.to_string_lossy()).map_err(hnsw_err)?;
        let meta = serde_json::to_string(&(&self.config, self.dimensions)).map_err(json_err)?;
        let meta_path = Self::meta_path(path);
        std::fs::write(&meta_path, meta).map_err(|e| MemoryError::IoError {
            path: meta_path.display().to_string(),
            source: e,
        })
    }

    /// Get index statistics.
    pub fn statistics(&self) -> HnswStats {
        HnswStats {
            nodes_total: self.index.size() as u64,
            dimensions: self.dimensions,
            connectivity: self.index.connectivity(),
            memory_bytes: self.index.memory_usage() as u64,
        }
    }
}
