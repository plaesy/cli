//! `impl Database` methods: documents, split out of `mod.rs`.

use super::*;

impl Database {
    /// Whether any indexed document still needs a content or embedding sync.
    pub async fn has_unindexed_documents(&self) -> Result<bool> {
        let conn = self.conn.lock();
        let pending = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE hash IS NULL)",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(db_err)?;
        Ok(pending != 0)
    }

    /// Atomically replace a document, its links, chunks, and embeddings.
    ///
    /// The document hash is committed only together with all chunks and vectors,
    /// so an interrupted replacement cannot make the next reconciliation skip it.
    pub async fn replace_document(
        &self,
        doc: &Document,
        chunks: &[Chunk],
        embeddings: Option<&[EmbeddingVector]>,
    ) -> Result<(u64, Vec<u64>)> {
        if embeddings.is_some_and(|vectors| vectors.len() != chunks.len()) {
            return Err(MemoryError::InternalError(
                "chunks and embeddings length mismatch".into(),
            ));
        }
        let path = doc.source_path.to_string_lossy().replace('\\', "/");
        let metadata = serde_json::to_string(&doc.metadata).map_err(json_err)?;
        let revision = doc
            .metadata
            .get("revision")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let indexed_at = doc.indexed_at.unwrap_or_else(chrono::Utc::now).to_rfc3339();
        let conn = self.conn.lock();
        conn.execute_batch("SAVEPOINT replace_document")
            .map_err(db_err)?;
        let res: rusqlite::Result<(u64, Vec<u64>)> = (|| {
            let id: i64 = conn.query_row(
                "INSERT INTO documents(source_path, hash, revision, metadata, body, indexed_at, body_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(source_path) DO UPDATE SET
                    hash = excluded.hash, revision = excluded.revision,
                    metadata = excluded.metadata, body = excluded.body,
                    indexed_at = excluded.indexed_at, body_start = excluded.body_start
                 RETURNING id",
                params![path, doc.hash, revision, metadata, doc.body, indexed_at, doc.body_start_line.max(1) as i64],
                |r| r.get(0),
            )?;
            conn.execute("DELETE FROM chunks WHERE document_id = ?1", params![id])?;
            conn.execute("DELETE FROM links WHERE document_id = ?1", params![id])?;
            for (target, line) in extract_wikilinks(&doc.body) {
                let file_line = line + doc.body_start_line.saturating_sub(1);
                conn.execute(
                    "INSERT INTO links(document_id, target, line) VALUES (?1, ?2, ?3)",
                    params![id, target, file_line as i64],
                )?;
            }
            let mut stmt = conn.prepare_cached(
                "INSERT INTO chunks(document_id, text, kind, layer, start_line, end_line, metadata, embedding)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            let mut ids = Vec::with_capacity(chunks.len());
            for (index, chunk) in chunks.iter().enumerate() {
                let meta = chunk
                    .metadata
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let embedding = embeddings
                    .and_then(|vectors| vectors.get(index))
                    .map(|v| vec_to_blob(v));
                stmt.execute(params![
                    id,
                    chunk.text,
                    kind_to_str(chunk.kind),
                    chunk.layer as i64,
                    chunk.start_line as i64,
                    chunk.end_line as i64,
                    meta,
                    embedding
                ])?;
                ids.push(conn.last_insert_rowid() as u64);
            }
            bump_generation(&conn)?;
            Ok((id as u64, ids))
        })();
        match res {
            Ok(value) => {
                conn.execute_batch("RELEASE replace_document")
                    .map_err(db_err)?;
                Ok(value)
            }
            Err(error) => {
                let _ =
                    conn.execute_batch("ROLLBACK TO replace_document; RELEASE replace_document");
                Err(db_err(error))
            }
        }
    }

    /// Insert or update a document (keyed by `source_path`).
    ///
    /// On update, all previous chunks of the document are removed (and with
    /// them their FTS rows); callers re-insert chunks afterwards. Returns the
    /// document row ID.
    pub async fn insert_document(&self, doc: &Document) -> Result<u64> {
        let path = doc.source_path.to_string_lossy().replace('\\', "/");
        let metadata = serde_json::to_string(&doc.metadata).map_err(json_err)?;
        let revision = doc
            .metadata
            .get("revision")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let indexed_at = doc.indexed_at.unwrap_or_else(chrono::Utc::now).to_rfc3339();
        let conn = self.conn.lock();
        conn.execute_batch("SAVEPOINT insert_document")
            .map_err(db_err)?;
        let res: rusqlite::Result<i64> = (|| {
            let id: i64 = conn.query_row(
                "INSERT INTO documents(source_path, hash, revision, metadata, body, indexed_at, body_start)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(source_path) DO UPDATE SET
                    hash = excluded.hash, revision = excluded.revision,
                    metadata = excluded.metadata, body = excluded.body,
                    indexed_at = excluded.indexed_at, body_start = excluded.body_start
                 RETURNING id",
                params![
                    path,
                    doc.hash,
                    revision,
                    metadata,
                    doc.body,
                    indexed_at,
                    doc.body_start_line.max(1) as i64
                ],
                |r| r.get(0),
            )?;
            conn.execute("DELETE FROM chunks WHERE document_id = ?1", params![id])?;
            conn.execute("DELETE FROM links WHERE document_id = ?1", params![id])?;
            for (target, line) in extract_wikilinks(&doc.body) {
                // `line` is relative to the body; report it relative to the file.
                let file_line = line + doc.body_start_line.saturating_sub(1);
                conn.execute(
                    "INSERT INTO links(document_id, target, line) VALUES (?1, ?2, ?3)",
                    params![id, target, file_line as i64],
                )?;
            }
            bump_generation(&conn)?;
            Ok(id)
        })();
        match res {
            Ok(id) => {
                conn.execute_batch("RELEASE insert_document")
                    .map_err(db_err)?;
                Ok(id as u64)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK TO insert_document; RELEASE insert_document");
                Err(db_err(e))
            }
        }
    }

    /// Insert a batch of chunks belonging to `document_id`.
    ///
    /// Returns the assigned chunk IDs in input order. The batch is atomic.
    pub async fn insert_chunks(&self, document_id: u64, chunks: &[Chunk]) -> Result<Vec<u64>> {
        let conn = self.conn.lock();
        conn.execute_batch("SAVEPOINT insert_chunks")
            .map_err(db_err)?;
        let res: Result<Vec<u64>> = (|| {
            let mut stmt = conn
                .prepare_cached(
                    "INSERT INTO chunks(document_id, text, kind, layer, start_line, end_line, metadata)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(db_err)?;
            let mut ids = Vec::with_capacity(chunks.len());
            for c in chunks {
                let meta = match &c.metadata {
                    Some(m) => Some(serde_json::to_string(m).map_err(json_err)?),
                    None => None,
                };
                stmt.execute(params![
                    document_id as i64,
                    c.text,
                    kind_to_str(c.kind),
                    c.layer as i64,
                    c.start_line as i64,
                    c.end_line as i64,
                    meta
                ])
                .map_err(db_err)?;
                ids.push(conn.last_insert_rowid() as u64);
            }
            Ok(ids)
        })();
        match res {
            Ok(ids) => {
                conn.execute_batch("RELEASE insert_chunks")
                    .map_err(db_err)?;
                Ok(ids)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK TO insert_chunks; RELEASE insert_chunks");
                Err(e)
            }
        }
    }

    /// Update chunk embeddings (vectors).
    ///
    /// # Errors
    ///
    /// - `InternalError` if the slices differ in length
    pub async fn update_chunk_embeddings(
        &self,
        chunk_ids: &[u64],
        embeddings: &[EmbeddingVector],
    ) -> Result<()> {
        if chunk_ids.len() != embeddings.len() {
            return Err(MemoryError::InternalError(format!(
                "chunk_ids ({}) and embeddings ({}) length mismatch",
                chunk_ids.len(),
                embeddings.len()
            )));
        }
        let conn = self.conn.lock();
        conn.execute_batch("SAVEPOINT update_emb").map_err(db_err)?;
        let res: rusqlite::Result<()> = (|| {
            let mut stmt = conn.prepare_cached("UPDATE chunks SET embedding = ?1 WHERE id = ?2")?;
            for (id, v) in chunk_ids.iter().zip(embeddings) {
                stmt.execute(params![vec_to_blob(v), *id as i64])?;
            }
            bump_generation(&conn)?;
            Ok(())
        })();
        match res {
            Ok(()) => conn.execute_batch("RELEASE update_emb").map_err(db_err),
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK TO update_emb; RELEASE update_emb");
                Err(db_err(e))
            }
        }
    }

    /// Look up a document by source path: returns `(id, content hash)`.
    pub async fn find_document(&self, source_path: &str) -> Result<Option<(u64, Option<String>)>> {
        let path = source_path.replace('\\', "/");
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, hash FROM documents WHERE source_path = ?1",
            params![path],
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)),
        )
        .optional()
        .map_err(db_err)
    }

    /// All stored documents as `(id, source_path)`, ordered by path.
    pub async fn document_paths(&self) -> Result<Vec<(u64, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT id, source_path FROM documents ORDER BY source_path")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)))
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Group notes with identical content hashes for a read-only curation report.
    pub async fn exact_duplicate_documents(&self) -> Result<Vec<Vec<String>>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT hash, source_path FROM documents
                 WHERE hash IS NOT NULL ORDER BY hash, source_path",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(db_err)?;
        let mut groups: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let (hash, path) = row.map_err(db_err)?;
            groups.entry(hash).or_default().push(path);
        }
        Ok(groups
            .into_values()
            .filter(|paths| paths.len() > 1)
            .collect())
    }

    /// IDs of all chunks belonging to a document.
    pub async fn chunk_ids_for_document(&self, doc_id: u64) -> Result<Vec<u64>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT id FROM chunks WHERE document_id = ?1 ORDER BY id")
            .map_err(db_err)?;
        let rows = stmt
            .query_map(params![doc_id as i64], |r| Ok(r.get::<_, i64>(0)? as u64))
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Delete a document and its chunks (FK cascade; FTS rows removed by trigger).
    pub async fn delete_document(&self, doc_id: u64) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM documents WHERE id = ?1",
            params![doc_id as i64],
        )
        .map_err(db_err)?;
        bump_generation(&conn).map_err(db_err)?;
        Ok(())
    }

    /// Frontmatter metadata of the given documents, keyed by `source_path`.
    pub async fn document_metadata(
        &self,
        paths: &[String],
    ) -> Result<HashMap<String, HashMap<String, serde_json::Value>>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare_cached("SELECT metadata FROM documents WHERE source_path = ?1")
            .map_err(db_err)?;
        let mut out = HashMap::new();
        for p in paths {
            let raw: Option<String> = stmt
                .query_row(params![p], |r| r.get(0))
                .optional()
                .map_err(db_err)?;
            if let Some(raw) = raw {
                out.insert(p.clone(), serde_json::from_str(&raw).unwrap_or_default());
            }
        }
        Ok(out)
    }

    /// List document metadata without loading note bodies, in stable path order.
    pub async fn list_documents(
        &self,
        path_prefix: Option<&str>,
        note_type: Option<&str>,
        status: Option<&str>,
        query: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<ListedDocument>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT source_path, metadata FROM documents
                 WHERE (?1 IS NULL OR substr(source_path, 1, length(?1)) = ?1)
                   AND (?2 IS NULL OR lower(COALESCE(json_extract(metadata, '$.type'), json_extract(metadata, '$.note_type'), '')) = lower(?2))
                   AND (?3 IS NULL OR lower(COALESCE(json_extract(metadata, '$.status'), '')) = lower(?3))
                   AND (?4 IS NULL OR instr(lower(source_path), lower(?4)) > 0 OR instr(lower(body), lower(?4)) > 0)
                 ORDER BY source_path LIMIT ?5 OFFSET ?6",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map(
                params![
                    path_prefix,
                    note_type,
                    status,
                    query,
                    limit as i64,
                    offset as i64
                ],
                |r| {
                    let raw: String = r.get(1)?;
                    Ok(ListedDocument {
                        path: r.get(0)?,
                        metadata: serde_json::from_str(&raw).unwrap_or_default(),
                    })
                },
            )
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Count indexed documents matching the list filters.
    pub async fn count_documents(
        &self,
        path_prefix: Option<&str>,
        note_type: Option<&str>,
        status: Option<&str>,
        query: Option<&str>,
    ) -> Result<u64> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM documents
             WHERE (?1 IS NULL OR substr(source_path, 1, length(?1)) = ?1)
               AND (?2 IS NULL OR lower(COALESCE(json_extract(metadata, '$.type'), json_extract(metadata, '$.note_type'), '')) = lower(?2))
               AND (?3 IS NULL OR lower(COALESCE(json_extract(metadata, '$.status'), '')) = lower(?3))
               AND (?4 IS NULL OR instr(lower(source_path), lower(?4)) > 0 OR instr(lower(body), lower(?4)) > 0)",
            params![path_prefix, note_type, status, query],
            |r| r.get(0),
        )
        .map_err(db_err)
    }

    /// Every stored document, ordered by path.
    pub async fn all_documents(&self) -> Result<Vec<StoredDocument>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT source_path, metadata, body, body_start FROM documents ORDER BY source_path",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                let raw: String = r.get(1)?;
                Ok(StoredDocument {
                    path: r.get(0)?,
                    metadata: serde_json::from_str(&raw).unwrap_or_default(),
                    body: r.get(2)?,
                    body_start_line: r.get::<_, i64>(3)?.max(1) as usize,
                })
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }
}
