//! `impl Database` methods: retrieval, split out of `mod.rs`.

use super::*;

impl Database {
    /// Search using full-text search (FTS5, BM25 ranking).
    ///
    /// Scores are BM25 squashed into (0, 1) via `s / (s + 8)` (absolute, not relative to the best hit). Every hit is
    /// verbatim stored text containing a query term, so confidence is `Fact`.
    pub async fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let Some(expr) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let conn = self.conn.lock();
        // bm25() is negative (more negative = better), so ORDER BY ascending.
        let sql = "SELECT c.id, c.text, d.source_path, c.start_line, c.end_line, \
                   d.revision, d.indexed_at, f.r \
                   FROM (SELECT rowid AS rid, bm25(chunks_fts) AS r FROM chunks_fts \
                         WHERE chunks_fts MATCH ?1 ORDER BY r LIMIT ?2) f \
                   JOIN chunks c ON c.id = f.rid \
                   JOIN documents d ON d.id = c.document_id ORDER BY f.r";
        let kerr = |e: rusqlite::Error| MemoryError::KeywordSearchError(e.to_string());
        let mut stmt = conn.prepare(sql).map_err(kerr)?;
        let rows = stmt
            .query_map(params![expr, limit as i64], |row| {
                let r: f64 = row.get(7)?;
                let s = (-r).max(0.0);
                row_to_result(
                    row,
                    (s / (s + BM25_HALF_SATURATION)) as f32,
                    ConfidenceLevel::Fact,
                )
            })
            .map_err(kerr)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(kerr)
    }

    /// Build a `SearchResult` (with full citation data) for a stored chunk.
    pub async fn result_for_chunk(
        &self,
        chunk_id: u64,
        score: f32,
        confidence: ConfidenceLevel,
    ) -> Result<Option<SearchResult>> {
        let conn = self.conn.lock();
        conn.query_row(
            &format!("{RESULT_SELECT} WHERE c.id = ?1"),
            params![chunk_id as i64],
            |row| row_to_result(row, score, confidence),
        )
        .optional()
        .map_err(db_err)
    }

    fn row_to_chunk(row: &rusqlite::Row<'_>) -> rusqlite::Result<Chunk> {
        let meta: Option<String> = row.get(7)?;
        let metadata: Option<HashMap<String, String>> =
            meta.and_then(|m| serde_json::from_str(&m).ok());
        Ok(Chunk {
            id: Some(row.get::<_, i64>(0)? as u64),
            document_id: Some(row.get::<_, i64>(1)? as u64),
            text: row.get(2)?,
            kind: kind_from_str(&row.get::<_, String>(3)?),
            layer: row.get::<_, i64>(4)? as usize,
            start_line: row.get::<_, i64>(5)? as usize,
            end_line: row.get::<_, i64>(6)? as usize,
            metadata,
        })
    }

    const CHUNK_COLS: &'static str =
        "id, document_id, text, kind, layer, start_line, end_line, metadata";

    /// Retrieve a chunk by ID.
    pub async fn get_chunk(&self, chunk_id: u64) -> Result<Option<Chunk>> {
        let conn = self.conn.lock();
        conn.query_row(
            &format!("SELECT {} FROM chunks WHERE id = ?1", Self::CHUNK_COLS),
            params![chunk_id as i64],
            Self::row_to_chunk,
        )
        .optional()
        .map_err(db_err)
    }

    /// Get all chunks for a document, in document order.
    pub async fn get_document_chunks(&self, doc_id: u64) -> Result<Vec<Chunk>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM chunks WHERE document_id = ?1 ORDER BY id",
                Self::CHUNK_COLS
            ))
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![doc_id as i64], Self::row_to_chunk)
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Sorted IDs of all chunks that have a stored embedding.
    /// This identity manifest is used to validate the persisted HNSW cache.
    pub async fn embedding_ids(&self) -> Result<Vec<u64>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT id FROM chunks WHERE embedding IS NOT NULL ORDER BY id")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, i64>(0).map(|id| id as u64))
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Number of chunks that have a stored embedding.
    pub async fn embedding_count(&self) -> Result<u64> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE embedding IS NOT NULL",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n as u64)
        .map_err(db_err)
    }

    /// All stored `(chunk_id, embedding)` pairs (used to rebuild the HNSW index).
    pub async fn all_embeddings(&self) -> Result<Vec<(u64, EmbeddingVector)>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT id, embedding FROM chunks WHERE embedding IS NOT NULL ORDER BY id")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    blob_to_vec(&r.get::<_, Vec<u8>>(1)?),
                ))
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// All wikilinks with their resolution against the indexed documents.
    pub async fn all_links(&self) -> Result<Vec<LinkRef>> {
        let (raw, paths): (Vec<(String, String, usize)>, Vec<String>) = {
            let conn = self.conn.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT d.source_path, l.target, l.line FROM links l                      JOIN documents d ON d.id = l.document_id ORDER BY d.source_path, l.line",
                )
                .map_err(db_err)?;
            let raw = stmt
                .query_map([], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as usize))
                })
                .map_err(db_err)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(db_err)?;
            let mut pstmt = conn
                .prepare("SELECT source_path FROM documents")
                .map_err(db_err)?;
            let paths = pstmt
                .query_map([], |r| r.get(0))
                .map_err(db_err)?
                .collect::<rusqlite::Result<Vec<String>>>()
                .map_err(db_err)?;
            (raw, paths)
        };
        Ok(raw
            .into_iter()
            .filter(|(_, target, _)| plausible_target(target))
            .map(|(source_path, target, line)| {
                let resolved_path = resolve_link(&target, &paths);
                LinkRef {
                    source_path,
                    target,
                    line,
                    resolved_path,
                }
            })
            .collect())
    }

    /// Get database statistics (diagnostic).
    ///
    /// `hnsw_size_bytes` and `coverage_percent` are not known to the database
    /// and are reported as 0; callers owning the HNSW index / file tree fill them.
    pub async fn statistics(&self) -> Result<DatabaseStats> {
        let conn = self.conn.lock();
        let count = |sql: &str| -> Result<u64> {
            conn.query_row(sql, [], |r| r.get::<_, i64>(0))
                .map(|n| n as u64)
                .map_err(db_err)
        };
        let documents_total = count("SELECT COUNT(*) FROM documents")?;
        let chunks_total = count("SELECT COUNT(*) FROM chunks")?;
        let page_count = count("PRAGMA page_count")?;
        let page_size = count("PRAGMA page_size")?;
        let fts_index_size_bytes = if self.config.enable_fts {
            count("SELECT COALESCE(SUM(LENGTH(block)), 0) FROM chunks_fts_data")?
        } else {
            0
        };
        Ok(DatabaseStats {
            documents_total,
            chunks_total,
            db_size_bytes: page_count * page_size,
            fts_index_size_bytes,
            hnsw_size_bytes: 0,
            coverage_percent: 0.0,
        })
    }
}
