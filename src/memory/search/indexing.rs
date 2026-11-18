//! `impl SearchEngine` methods: indexing, split out of `mod.rs`.

use super::*;

impl SearchEngine {
    /// Index (or re-index) a document, replacing any previous version.
    ///
    /// Embedding happens first; if it fails the document is still stored for
    /// keyword search and `embedded` is false.
    pub async fn index_document(&self, mut doc: Document) -> Result<IndexOutcome> {
        let timer = Timer::start();
        doc.source_path = PathBuf::from(doc.source_path.to_string_lossy().replace('\\', "/"));
        let path = doc.source_path.to_string_lossy().to_string();
        let mut chunks = self.chunker.chunk(&doc)?;
        if chunks.is_empty() {
            // Nothing indexable (e.g. only blank lines/frontmatter-less empty body).
            chunks = Vec::new();
        }

        let mut vectors: Option<Vec<EmbeddingVector>> = None;
        if let (Some(engine), true) = (&self.embeddings, self.config.enable_vector_search) {
            let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
            let t = Timer::start();
            match engine.embed_batch(&texts).await {
                Ok(v) => {
                    self.metrics.record_embedding_latency(t.elapsed());
                    self.metrics
                        .set_embedding_cache_hit_ratio(engine.cache_hit_ratio());
                    vectors = Some(v);
                }
                Err(e) => {
                    self.metrics.record_error("embedding_failed");
                    tracing::warn!("embedding failed for {path}, indexing keyword-only: {e}");
                }
            }
        }

        let _guard = self.write_lock.lock().await;
        let old_chunks = if let Some((old_id, _)) = self.db.find_document(&path).await? {
            self.db.chunk_ids_for_document(old_id).await?
        } else {
            Vec::new()
        };
        // Commit the complete SQLite replacement first. The hash is not published until
        // chunks and embeddings are present, so a later reconciliation can repair failures.
        let (_doc_id, ids) = self
            .db
            .replace_document(&doc, &chunks, vectors.as_deref())
            .await?;
        // HNSW is a disposable cache; update it only after SQLite has committed.
        let mut h = self.hnsw.write();
        for id in old_chunks {
            h.remove(id)?;
        }
        if let Some(vecs) = &vectors {
            for (id, v) in ids.iter().zip(vecs) {
                h.insert(*id, v)?;
            }
        }
        self.metrics.record_document_indexed(chunks.len());
        self.metrics
            .record_indexing_latency(timer.elapsed(), "index_document");
        Ok(IndexOutcome::Indexed {
            chunks: chunks.len(),
            embedded: vectors.is_some(),
        })
    }

    /// Index one file. `source_path` is stored relative to `root` (forward slashes).
    /// Skips the write when the content hash is unchanged.
    pub async fn index_file(&self, root: &Path, path: &Path) -> Result<IndexOutcome> {
        let _process_lock = if self.daemon_mode() {
            None
        } else {
            Some(WriteLock::acquire(root)?)
        };
        self.index_file_at(root, path, git_revision(root)).await
    }

    /// Like [`SearchEngine::index_file`] with a caller-supplied git revision
    /// (spawning `git` per file is slow, so directory syncs resolve it once).
    pub(crate) async fn index_file_at(
        &self,
        root: &Path,
        path: &Path,
        revision: Option<String>,
    ) -> Result<IndexOutcome> {
        let rel = path.strip_prefix(root).map_err(|_| {
            MemoryError::ConfigError(format!(
                "cannot index a file outside the notes root: {}",
                path.display()
            ))
        })?;
        if rel.components().any(|component| {
            matches!(
                component,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::ParentDir
            )
        }) {
            return Err(MemoryError::ConfigError(format!(
                "cannot index a file with an unsafe relative path: {}",
                path.display()
            )));
        }
        let mut doc = self.extractor.extract_file(path)?;
        doc.source_path = PathBuf::from(rel.to_string_lossy().replace('\\', "/"));
        let key = doc.source_path.to_string_lossy().to_string();
        let note_id = doc.metadata.get("id").and_then(serde_json::Value::as_str);
        let forgotten = crate::memory::lifecycle::is_forgotten(root, &key, note_id)?
            || match doc.hash.as_deref() {
                Some(hash) => crate::memory::lifecycle::content_hash_is_forgotten(root, hash)?,
                None => false,
            };
        if forgotten {
            if self.remove_path_locked(&key).await? {
                return Ok(IndexOutcome::Forgotten);
            }
            return Ok(IndexOutcome::Unchanged);
        }
        if let Some((_, Some(h))) = self.db.find_document(&key).await? {
            if Some(&h) == doc.hash.as_ref() {
                return Ok(IndexOutcome::Unchanged);
            }
        }
        if !doc.metadata.contains_key("revision") {
            if let Some(rev) = revision {
                doc.metadata
                    .insert("revision".to_string(), serde_json::Value::String(rev));
            }
        }
        self.index_document(doc).await
    }

    /// Sync a directory: index new/changed files, remove documents whose file is gone.
    /// Per-file failures are collected in the summary and do not abort the sync.
    pub async fn index_directory(&self, root: &Path) -> Result<IndexSummary> {
        let _process_lock = if self.daemon_mode() {
            None
        } else {
            Some(WriteLock::acquire(root)?)
        };
        reconcile_stale_temp_files(root);
        let mut summary = IndexSummary::default();
        // Replay durable note-publication work before the regular scan. This closes the window
        // between Markdown publication and SQLite/HNSW indexing across process crashes.
        for relative in self.db.pending_indexes().await? {
            let candidate = root.join(&relative);
            if candidate.is_file() {
                match self.index_file_at(root, &candidate, None).await {
                    Ok(_) => {
                        self.db.dequeue_index(&relative).await?;
                    }
                    Err(e) => {
                        tracing::warn!("outbox replay failed for {relative}: {e}");
                    }
                }
            } else {
                // The file was removed after enqueue; it cannot be replayed, so discard the
                // stale work item and let the normal removal reconciliation handle the index.
                self.db.dequeue_index(&relative).await?;
            }
        }
        let files: Vec<PathBuf> = scan_dir(root)
            .into_iter()
            .filter(|f| !is_excluded(root, f, &self.config.exclude_dirs))
            .collect();
        let mut present: HashSet<String> = HashSet::new();
        let revision = git_revision(root);
        for file in &files {
            let rel = file
                .strip_prefix(root)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            present.insert(rel.clone());
            match self.index_file_at(root, file, revision.clone()).await {
                Ok(IndexOutcome::Indexed { chunks, .. }) => {
                    summary.indexed += 1;
                    summary.chunks += chunks;
                }
                Ok(IndexOutcome::Unchanged) => summary.unchanged += 1,
                Ok(IndexOutcome::Forgotten) => summary.removed += 1,
                Err(e) => {
                    self.metrics.record_error("index_file_failed");
                    summary.failed.push((rel, e.to_string()));
                }
            }
        }
        for (_, stored) in self.db.document_paths().await? {
            if !present.contains(&stored) && self.remove_path_locked(&stored).await? {
                summary.removed += 1;
            }
        }
        if self.config.enable_vector_search && self.embeddings.is_some() {
            let needs_reindex = self.db.has_unindexed_documents().await?;
            if !needs_reindex && !self.vector_index_ready.load(Ordering::Acquire) {
                self.rebuild_hnsw_from_storage().await?;
                self.vector_index_ready.store(true, Ordering::Release);
            }
        }
        self.save_snapshot_locked().await;
        Ok(summary)
    }

    /// Remove a document (by stored relative path) from both stores.
    /// Returns whether it existed.
    pub async fn remove_path(&self, source_path: &str) -> Result<bool> {
        let _process_lock = if self.daemon_mode() {
            None
        } else {
            self.watch_root().map(WriteLock::acquire).transpose()?
        };
        self.remove_path_locked(source_path).await
    }

    /// Hash the current note bytes for an optimistic, conflict-checked update.
    pub fn fingerprint_note(&self, relative_path: &Path) -> Result<String> {
        let root = self
            .watch_root()
            .ok_or_else(|| MemoryError::ConfigError("no notes root is configured".into()))?;
        fingerprint_note_in_root(root, relative_path)
    }

    /// Replace a note only when its current SHA-256 matches the caller's reviewed version.
    ///
    /// The replacement is published atomically, recorded without retaining note content, and
    /// reconciled with the local index through its durable indexing outbox.
    pub async fn update_note(
        &self,
        relative_path: &Path,
        expected_sha256: &str,
        updated_content: &str,
        reason: &str,
    ) -> Result<String> {
        self.replace_note(
            relative_path,
            expected_sha256,
            updated_content,
            NoteLifecycleOperation::Update {
                reason: reason.to_owned(),
            },
        )
        .await
    }

    /// Mark a note archived without moving or deleting its source file.
    ///
    /// Its status is set to `archived`, so default retrieval hides it while `--include-inactive`
    /// can still return it. The archive event stores hashes and a structured reason only.
    pub async fn archive_note(
        &self,
        relative_path: &Path,
        expected_sha256: &str,
        reason: &str,
    ) -> Result<String> {
        let root = self
            .watch_root()
            .ok_or_else(|| MemoryError::ConfigError("no notes root is configured".into()))?
            .to_path_buf();
        let current_hash = fingerprint_note_in_root(&root, relative_path)?;
        if current_hash != expected_sha256 {
            return Err(MemoryError::ConfigError(format!(
                "note changed since it was reviewed (current SHA-256: {current_hash}); fingerprint it again before archiving"
            )));
        }
        let source = root.join(relative_path);
        let content =
            std::fs::read_to_string(&source).map_err(|source_error| MemoryError::IoError {
                path: source.display().to_string(),
                source: source_error,
            })?;
        if crate::memory::indexer::content_hash(&content) != current_hash {
            return Err(MemoryError::ConfigError(
                "note changed while it was being read; fingerprint it again before archiving"
                    .into(),
            ));
        }
        let document = self.extractor.extract(source.clone(), &content)?;
        let from_state = document
            .metadata
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("active")
            .to_owned();
        if from_state.eq_ignore_ascii_case("archived") {
            return Err(MemoryError::ConfigError("note is already archived".into()));
        }
        let archived_content = archived_note_content(&content, &document)?;
        self.replace_note(
            relative_path,
            expected_sha256,
            &archived_content,
            NoteLifecycleOperation::Archive {
                reason: reason.to_owned(),
                from_state,
            },
        )
        .await
    }

    async fn replace_note(
        &self,
        relative_path: &Path,
        expected_sha256: &str,
        updated_content: &str,
        operation: NoteLifecycleOperation,
    ) -> Result<String> {
        use std::io::Write;
        use std::path::Component;

        if updated_content.trim().is_empty() || updated_content.len() > 256 * 1024 {
            return Err(MemoryError::ConfigError(
                "updated note must be non-empty and at most 256 KiB".into(),
            ));
        }
        if expected_sha256.len() != 64
            || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(MemoryError::ConfigError(
                "--if-match must be a 64-character SHA-256 hex digest".into(),
            ));
        }
        if relative_path.as_os_str().is_empty()
            || relative_path.is_absolute()
            || relative_path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(MemoryError::ConfigError(
                "update requires a relative file path inside the vault".into(),
            ));
        }
        let root = self
            .watch_root()
            .ok_or_else(|| MemoryError::ConfigError("no notes root is configured".into()))?
            .to_path_buf();
        let rel = relative_path.to_string_lossy().replace('\\', "/");
        let _process_lock = if self.daemon_mode() {
            None
        } else {
            Some(WriteLock::acquire(&root)?)
        };
        let notes_root = std::fs::canonicalize(&root).map_err(|source| MemoryError::IoError {
            path: root.display().to_string(),
            source,
        })?;
        let source = notes_root.join(relative_path);
        let parent = source.parent().ok_or_else(|| {
            MemoryError::ConfigError("update path has no parent directory".into())
        })?;
        let real_parent =
            std::fs::canonicalize(parent).map_err(|source_error| MemoryError::IoError {
                path: parent.display().to_string(),
                source: source_error,
            })?;
        if !real_parent.starts_with(&notes_root) {
            return Err(MemoryError::ConfigError(format!(
                "update path resolves outside the vault: {}",
                source.display()
            )));
        }
        let meta =
            std::fs::symlink_metadata(&source).map_err(|source_error| MemoryError::IoError {
                path: source.display().to_string(),
                source: source_error,
            })?;
        if !meta.file_type().is_file() {
            return Err(MemoryError::ConfigError(format!(
                "update refuses a non-regular file: {}",
                source.display()
            )));
        }
        let real_source =
            std::fs::canonicalize(&source).map_err(|source_error| MemoryError::IoError {
                path: source.display().to_string(),
                source: source_error,
            })?;
        if !real_source.starts_with(&notes_root) {
            return Err(MemoryError::ConfigError(format!(
                "update path resolves outside the vault: {}",
                source.display()
            )));
        }
        let source_permissions = std::fs::metadata(&real_source)
            .map_err(|source_error| MemoryError::IoError {
                path: real_source.display().to_string(),
                source: source_error,
            })?
            .permissions();
        let current = self.extractor.extract_file(&real_source)?;
        let current_hash = current
            .hash
            .as_deref()
            .ok_or_else(|| MemoryError::ConfigError("could not fingerprint current note".into()))?;
        if current_hash != expected_sha256 {
            return Err(MemoryError::ConfigError(format!(
                "note changed since it was reviewed (current SHA-256: {current_hash}); fingerprint it again before updating"
            )));
        }
        let updated = self
            .extractor
            .extract(real_source.clone(), updated_content)?;
        let current_id = current
            .metadata
            .get("id")
            .and_then(serde_json::Value::as_str);
        let updated_id = updated
            .metadata
            .get("id")
            .and_then(serde_json::Value::as_str);
        if current_id != updated_id {
            return Err(MemoryError::ConfigError(
                "update cannot add, remove, or change a note's stable id".into(),
            ));
        }
        if crate::memory::lifecycle::is_forgotten(&notes_root, &rel, current_id)?
            || crate::memory::lifecycle::content_hash_is_forgotten(&notes_root, current_hash)?
        {
            return Err(MemoryError::ConfigError(
                "cannot update a note that has already been forgotten".into(),
            ));
        }
        let updated_hash = updated
            .hash
            .as_deref()
            .ok_or_else(|| MemoryError::ConfigError("could not fingerprint updated note".into()))?;
        if crate::memory::lifecycle::content_hash_is_forgotten(&notes_root, updated_hash)? {
            return Err(MemoryError::ConfigError(
                "updated content matches a note that has already been forgotten".into(),
            ));
        }
        if current_hash == updated_hash {
            return Ok(current_hash.to_owned());
        }
        let scope = match current.metadata.get("scope") {
            Some(serde_json::Value::String(value)) => vec![value.clone()],
            Some(serde_json::Value::Array(values)) => values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        let intent = match operation {
            NoteLifecycleOperation::Update { reason } => {
                NoteLifecycleIntent::Update(crate::memory::lifecycle::begin_update(
                    &notes_root,
                    &rel,
                    current_id,
                    scope,
                    current_hash,
                    updated_hash,
                    &reason,
                )?)
            }
            NoteLifecycleOperation::Archive { reason, from_state } => {
                NoteLifecycleIntent::Archive(crate::memory::lifecycle::begin_archive(
                    &notes_root,
                    crate::memory::lifecycle::ArchiveTransition {
                        relative_path: &rel,
                        note_id: current_id,
                        scope,
                        from_hash: current_hash,
                        to_hash: updated_hash,
                        from_state: &from_state,
                        reason: &reason,
                    },
                )?)
            }
        };
        self.database().enqueue_index(&rel).await?;

        let file_name = source
            .file_name()
            .ok_or_else(|| MemoryError::ConfigError("update path must name a file".into()))?;
        let temp_name = format!(".plaesy-update-{}.tmp", uuid::Uuid::new_v4());
        let dir = crate::memory::fs_safety::DirHandle::open_nofollow(&real_parent).map_err(
            |source_error| MemoryError::IoError {
                path: real_parent.display().to_string(),
                source: source_error,
            },
        )?;
        let write_result = (|| -> std::io::Result<()> {
            let mut file = dir.create_new_file(Path::new(&temp_name))?;
            file.write_all(updated_content.as_bytes())?;
            file.sync_all()?;
            file.set_permissions(source_permissions)?;
            file.sync_all()?;
            drop(file);
            let latest = std::fs::read_to_string(&source)?;
            if crate::memory::indexer::content_hash(&latest) != expected_sha256 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "note changed during update; replacement was not published",
                ));
            }
            dir.replace_within(Path::new(&temp_name), Path::new(file_name))?;
            dir.sync()?;
            Ok(())
        })();
        if let Err(source_error) = write_result {
            let _ = dir.remove_file(Path::new(&temp_name));
            if source_error.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(MemoryError::ConfigError(
                    "note changed during update; no replacement was published. Fingerprint it again before retrying".into(),
                ));
            }
            return Err(MemoryError::IoError {
                path: source.display().to_string(),
                source: source_error,
            });
        }
        self.index_file_at(&notes_root, &real_source, None).await?;
        match intent {
            NoteLifecycleIntent::Update(intent) => {
                crate::memory::lifecycle::complete_update(&notes_root, intent)?;
            }
            NoteLifecycleIntent::Archive(intent) => {
                crate::memory::lifecycle::complete_archive(&notes_root, intent)?;
            }
        }
        self.database().dequeue_index(&rel).await?;
        self.save_snapshot_locked().await;
        Ok(updated_hash.to_owned())
    }

    /// Forget a note: persist a synced tombstone, unlink its source, and remove its index entry.
    ///
    /// `relative_path` must be a path below the configured vault root. The tombstone is durable
    /// before the unlink, so a crash or a Syncthing conflict copy cannot make an identified note
    /// searchable again after the next index pass.
    pub async fn forget_note(&self, relative_path: &Path, reason: &str) -> Result<()> {
        use std::path::Component;

        let root = self
            .watch_root()
            .ok_or_else(|| MemoryError::ConfigError("no notes root is configured".into()))?
            .to_path_buf();
        if relative_path.as_os_str().is_empty()
            || relative_path.is_absolute()
            || relative_path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(MemoryError::ConfigError(
                "forget requires a relative file path inside the vault".into(),
            ));
        }
        let rel = relative_path.to_string_lossy().replace('\\', "/");
        let _process_lock = if self.daemon_mode() {
            None
        } else {
            Some(WriteLock::acquire(&root)?)
        };
        let real_root = std::fs::canonicalize(&root).map_err(|e| MemoryError::IoError {
            path: root.display().to_string(),
            source: e,
        })?;
        let source = real_root.join(relative_path);
        let parent = source.parent().ok_or_else(|| {
            MemoryError::ConfigError("forget path has no parent directory".into())
        })?;
        let real_parent = std::fs::canonicalize(parent).map_err(|e| MemoryError::IoError {
            path: parent.display().to_string(),
            source: e,
        })?;
        let notes_root = std::fs::canonicalize(&root).map_err(|e| MemoryError::IoError {
            path: root.display().to_string(),
            source: e,
        })?;
        if !real_parent.starts_with(&notes_root) {
            return Err(MemoryError::ConfigError(format!(
                "forget path resolves outside the vault: {}",
                source.display()
            )));
        }
        let file_name = source
            .file_name()
            .ok_or_else(|| MemoryError::ConfigError("forget path must name a file".into()))?;
        let mut note_id = None;
        let mut scope = Vec::new();
        let mut forgotten_content_hash = None;
        match std::fs::symlink_metadata(&source) {
            Ok(meta) if meta.file_type().is_file() => {
                let real_source =
                    std::fs::canonicalize(&source).map_err(|e| MemoryError::IoError {
                        path: source.display().to_string(),
                        source: e,
                    })?;
                if !real_source.starts_with(&notes_root) {
                    return Err(MemoryError::ConfigError(format!(
                        "forget path resolves outside the vault: {}",
                        source.display()
                    )));
                }
                let document = self.extractor.extract_file(&source)?;
                forgotten_content_hash = document.hash;
                note_id = document
                    .metadata
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                if let Some(value) = document.metadata.get("scope") {
                    scope = match value {
                        serde_json::Value::String(value) => vec![value.clone()],
                        serde_json::Value::Array(values) => values
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .collect(),
                        _ => Vec::new(),
                    };
                }
            }
            Ok(_) => {
                return Err(MemoryError::ConfigError(format!(
                    "forget refuses a non-regular file: {}",
                    source.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(MemoryError::IoError {
                    path: source.display().to_string(),
                    source: error,
                });
            }
        }
        let intent = crate::memory::lifecycle::begin_forget(
            &notes_root,
            &rel,
            note_id.as_deref(),
            forgotten_content_hash.as_deref(),
            scope,
            reason,
        )?;
        let dir =
            crate::memory::fs_safety::DirHandle::open_nofollow(&real_parent).map_err(|e| {
                MemoryError::IoError {
                    path: real_parent.display().to_string(),
                    source: e,
                }
            })?;
        match dir.remove_file(Path::new(file_name)) {
            Ok(()) => {
                dir.sync().map_err(|e| MemoryError::IoError {
                    path: real_parent.display().to_string(),
                    source: e,
                })?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(MemoryError::IoError {
                    path: source.display().to_string(),
                    source: error,
                });
            }
        }
        self.remove_path_locked(&rel).await?;
        crate::memory::lifecycle::complete_forget(&notes_root, intent)?;
        self.save_snapshot_locked().await;
        Ok(())
    }

    async fn remove_path_locked(&self, source_path: &str) -> Result<bool> {
        let _guard = self.write_lock.lock().await;
        let Some((doc_id, _)) = self.db.find_document(source_path).await? else {
            return Ok(false);
        };
        let ids = self.db.chunk_ids_for_document(doc_id).await?;
        {
            let mut h = self.hnsw.write();
            for id in ids {
                h.remove(id)?;
            }
        }
        self.db.delete_document(doc_id).await?;
        Ok(true)
    }

    /// Apply a batch of watcher events under `root`. Returns the number applied;
    /// failures are logged and counted in metrics, not propagated.
    pub async fn apply_events(&self, root: &Path, events: &[FileEvent]) -> usize {
        let _process_lock = if self.daemon_mode() {
            None
        } else {
            match WriteLock::acquire(root) {
                Ok(lock) => Some(lock),
                Err(e) => {
                    self.metrics.record_error("apply_events_lock_failed");
                    tracing::warn!("failed to acquire index writer lock: {e}");
                    return 0;
                }
            }
        };
        let mut applied = 0;
        for ev in events {
            if !is_indexable(&ev.path) || is_excluded(root, &ev.path, &self.config.exclude_dirs) {
                continue;
            }
            let res = match ev.kind {
                EventKind::Added | EventKind::Modified => {
                    if ev.path.is_file() {
                        self.index_file_at(root, &ev.path, None).await.map(|_| ())
                    } else {
                        continue; // vanished between event and read
                    }
                }
                EventKind::Deleted => {
                    let rel = ev
                        .path
                        .strip_prefix(root)
                        .unwrap_or(&ev.path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    self.remove_path_locked(&rel).await.map(|_| ())
                }
            };
            match res {
                Ok(()) => applied += 1,
                Err(e) => {
                    self.metrics.record_error("apply_event_failed");
                    tracing::warn!(
                        "failed to apply {:?} for {}: {e}",
                        ev.kind,
                        ev.path.display()
                    );
                }
            }
        }
        self.save_snapshot_locked().await;
        applied
    }
}
