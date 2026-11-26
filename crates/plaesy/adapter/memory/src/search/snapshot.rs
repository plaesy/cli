//! `impl SearchEngine` methods: snapshot, split out of `mod.rs`.

use super::*;

impl SearchEngine {
    fn snapshot_path(db: &Database) -> Option<PathBuf> {
        db.path().map(|p| {
            let mut s = p.as_os_str().to_owned();
            s.push(".hnsw");
            PathBuf::from(s)
        })
    }

    /// Load the on-disk HNSW snapshot if it is provably in sync with the database:
    /// same change generation, same dimension, and one vector per stored embedding.
    /// Any mismatch or error returns `None` and the caller rebuilds from SQLite.
    pub(super) async fn load_snapshot(
        db: &Database,
        dim: usize,
        generation: u64,
    ) -> Option<HnswIndex> {
        let path = Self::snapshot_path(db)?;
        let gen_file = path.with_extension("hnsw.gen");
        let saved: u64 = std::fs::read_to_string(&gen_file)
            .ok()?
            .trim()
            .parse()
            .ok()?;
        if saved != generation {
            return None;
        }
        let index = HnswIndex::load(&path).ok()?;
        let stored_ids = db.embedding_ids().await.ok()?;
        let ids_file = path.with_extension("hnsw.ids");
        let saved_ids: Vec<u64> =
            serde_json::from_str(&std::fs::read_to_string(ids_file).ok()?).ok()?;
        (index.dimensions() == dim && index.len() == stored_ids.len() && saved_ids == stored_ids)
            .then_some(index)
    }

    /// Persist the HNSW index next to the database so the next start skips the rebuild.
    /// No-op for in-memory databases or when nothing changed since the last snapshot.
    /// Failures are logged, never fatal (the snapshot is only a cache).
    pub async fn save_snapshot(&self) {
        let _process_lock = if self.daemon_mode() {
            None
        } else if let Some(root) = self.watch_root() {
            match WriteLock::acquire(root) {
                Ok(lock) => Some(lock),
                Err(error) => {
                    // A live daemon owns the vault and writes its own snapshot, so skipping here
                    // is expected, not a warning.
                    tracing::debug!("snapshot writer lock unavailable: {error}");
                    return;
                }
            }
        } else {
            None
        };
        self.save_snapshot_locked().await;
    }

    /// Snapshot publication for callers that already hold the process-wide writer lock.
    pub(super) async fn save_snapshot_locked(&self) {
        if !self.has_vector_search() {
            return;
        }
        let Some(path) = Self::snapshot_path(&self.db) else {
            return;
        };
        let _guard = self.write_lock.lock().await;
        let Ok(generation) = self.db.generation().await else {
            return;
        };
        if self.snapshot_generation.load(Ordering::Relaxed) == generation {
            return;
        }
        // Save to a private basename first. The generation marker is published last, so a
        // crash at any earlier point makes the old snapshot fail validation and triggers rebuild.
        let tmp = path.with_extension(format!("hnsw.tmp-{}", std::process::id()));
        let tmp_meta = {
            let mut p = tmp.as_os_str().to_owned();
            p.push(".meta.json");
            PathBuf::from(p)
        };
        let marker = path.with_extension("hnsw.gen");
        let marker_tmp = marker.with_extension(format!("gen.tmp-{}", std::process::id()));
        let ids_path = path.with_extension("hnsw.ids");
        let ids_tmp = ids_path.with_extension(format!("ids.tmp-{}", std::process::id()));
        let Ok(embedding_ids) = self.db.embedding_ids().await else {
            return;
        };
        let saved = {
            let h = self.hnsw.read();
            h.save(&tmp)
        };
        match saved {
            Ok(()) => {
                let result = (|| -> std::io::Result<()> {
                    let meta = {
                        let mut p = path.as_os_str().to_owned();
                        p.push(".meta.json");
                        PathBuf::from(p)
                    };
                    // Invalidate the old marker before replacing any snapshot component. If the
                    // process crashes at any point below, startup must rebuild from SQLite rather
                    // than accepting a mixed-generation HNSW/data/manifest bundle.
                    match std::fs::remove_file(&marker) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                    // Replace the data, metadata, and identity manifest as complete files,
                    // then publish generation last.
                    std::fs::rename(&tmp, &path)?;
                    std::fs::rename(&tmp_meta, &meta)?;
                    std::fs::write(
                        &ids_tmp,
                        serde_json::to_vec(&embedding_ids)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?,
                    )?;
                    std::fs::rename(&ids_tmp, &ids_path)?;
                    std::fs::write(&marker_tmp, generation.to_string())?;
                    std::fs::rename(&marker_tmp, &marker)?;
                    Ok(())
                })();
                if result.is_ok() {
                    self.snapshot_generation
                        .store(generation, Ordering::Relaxed);
                } else {
                    let _ = std::fs::remove_file(&tmp);
                    let _ = std::fs::remove_file(&tmp_meta);
                    let _ = std::fs::remove_file(&ids_tmp);
                    let _ = std::fs::remove_file(&marker_tmp);
                    tracing::warn!("could not atomically publish HNSW snapshot");
                }
            }
            Err(e) => tracing::warn!("could not save HNSW snapshot: {e}"),
        }
    }

    pub(super) async fn rebuild_hnsw_from_storage(&self) -> Result<()> {
        let _guard = self.write_lock.lock().await;
        let dim = self
            .embeddings
            .as_ref()
            .map(|engine| engine.dimension())
            .unwrap_or(384);
        let mut rebuilt = HnswIndex::with_dimensions(self.db.hnsw_config().clone(), dim)?;
        for (id, vector) in self.db.all_embeddings().await? {
            rebuilt.insert(id, &vector)?;
        }
        *self.hnsw.write() = rebuilt;
        Ok(())
    }
}
