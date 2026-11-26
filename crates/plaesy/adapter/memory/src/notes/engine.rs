//! `engine` helpers split out of `mod.rs`.

use super::*;

impl SearchEngine {
    /// Create a new note under the notes root and index it.
    ///
    /// # Errors
    ///
    /// - `ConfigError` when no notes root is set, the tier or confidence is unknown, the folder
    ///   escapes the root, or a note with the same file name already exists
    /// - `IoError` when the file cannot be written
    pub async fn remember(&self, note: &NewNote) -> Result<PathBuf> {
        self.remember_with_markdown(note, None).await
    }

    /// Write a template-rendered Markdown note through the normal safe publishing and indexing path.
    pub(crate) async fn remember_rendered(
        &self,
        note: &NewNote,
        markdown: &str,
    ) -> Result<PathBuf> {
        self.remember_with_markdown(note, Some(markdown)).await
    }

    async fn remember_with_markdown(
        &self,
        note: &NewNote,
        markdown: Option<&str>,
    ) -> Result<PathBuf> {
        // An agent cannot vouch for its own note: a `high` claim is capped at `medium`, so a
        // poisoned or wrong note never reads as established fact. The owner may raise it by hand.
        let mut note = note.clone();
        if note.confidence == "high" {
            note.confidence = "medium".into();
        }
        let note = &note;
        let root = self
            .watch_root()
            .ok_or_else(|| MemoryError::ConfigError("no notes root is configured".into()))?
            .to_path_buf();
        let title = note.title.trim();
        if title.is_empty() {
            return Err(MemoryError::ConfigError("title must not be empty".into()));
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(MemoryError::ConfigError(format!(
                "title is longer than {MAX_TITLE_CHARS} characters"
            )));
        }
        if markdown.is_none() && note.body.trim().is_empty() {
            return Err(MemoryError::ConfigError("body must not be empty".into()));
        }
        if markdown.is_none() && note.body.len() > MAX_BODY_BYTES {
            return Err(MemoryError::ConfigError(format!(
                "body is larger than {MAX_BODY_BYTES} bytes"
            )));
        }
        if !TIERS.contains(&note.tier.as_str()) {
            return Err(MemoryError::ConfigError(format!(
                "unknown tier: {}",
                note.tier
            )));
        }
        if !CONFIDENCE.contains(&note.confidence.as_str()) {
            return Err(MemoryError::ConfigError(format!(
                "confidence must be one of high, medium, low (got {})",
                note.confidence
            )));
        }
        // Serialize the graph preflight with file admission across CLI processes. Without this,
        // two writers can both observe an unused graph id and create duplicate IDs.
        let _write_lock = if self.daemon_mode() {
            None
        } else {
            Some(WriteLock::acquire(&root)?)
        };
        if let Some(g) = &note.graph {
            if !crate::memory::graph::NOTE_TYPES.contains(&g.note_type.as_str()) {
                return Err(MemoryError::ConfigError(format!(
                    "unknown graph type: {} (expected one of {:?})",
                    g.note_type,
                    crate::memory::graph::NOTE_TYPES
                )));
            }
            if g.id.trim().is_empty() {
                return Err(MemoryError::ConfigError(
                    "a graph note (--type given) needs a non-empty --id".into(),
                ));
            }
            if let Some(status) = &g.status {
                let allowed: &[&str] = match g.note_type.as_str() {
                    "project" => &crate::memory::graph::PROJECT_STATUSES,
                    "decision" => &crate::memory::graph::DECISION_STATUSES,
                    "context" => &crate::memory::graph::CONTEXT_STATUSES,
                    _ => &[],
                };
                if !allowed.is_empty() && !allowed.contains(&status.to_lowercase().as_str()) {
                    return Err(MemoryError::ConfigError(format!(
                        "status '{status}' is not valid for type '{}' (expected one of {allowed:?})",
                        g.note_type
                    )));
                }
            }
            // A decision cannot supersede itself: that would be a self-referencing cycle, not a
            // legitimate replacement, and nothing downstream would otherwise catch it.
            if g.supersedes.iter().any(|s| s == &g.id) {
                return Err(MemoryError::ConfigError(format!(
                    "decision '{}' cannot supersede itself",
                    g.id
                )));
            }
            // Two notes (any type) declaring the same id make every id lookup ambiguous: the
            // first match silently wins and every other claimant becomes invisible. Reject at
            // write time rather than letting it surface later as a silent lookup bug.
            let graph = crate::memory::graph::load_graph(&self.database()).await?;
            if let Some(existing) = graph.notes.iter().find(|n| n.id == g.id) {
                return Err(MemoryError::ConfigError(format!(
                    "id '{}' is already used by {}",
                    g.id, existing.path
                )));
            }
        }
        let rel = if note.folder.trim().is_empty() {
            default_inbox(&root)
        } else {
            safe_folder(&note.folder)?
        };
        let dir = root.join(rel);
        std::fs::create_dir_all(&dir).map_err(|e| MemoryError::IoError {
            path: dir.display().to_string(),
            source: e,
        })?;
        // A junction or symlink inside the root can point elsewhere; check where the folder really is.
        let real_root = std::fs::canonicalize(&root).map_err(|e| MemoryError::IoError {
            path: root.display().to_string(),
            source: e,
        })?;
        let real_dir = std::fs::canonicalize(&dir).map_err(|e| MemoryError::IoError {
            path: dir.display().to_string(),
            source: e,
        })?;
        if !real_dir.starts_with(&real_root) {
            return Err(MemoryError::ConfigError(format!(
                "folder resolves outside the notes root: {}",
                real_dir.display()
            )));
        }
        let path = dir.join(format!("{}.md", slugify(title)));
        let relative = path
            .strip_prefix(&root)
            .map_err(|_| MemoryError::ConfigError("note destination escaped notes root".into()))?
            .to_string_lossy()
            .replace('\\', "/");
        let candidate_id = note.graph.as_ref().map(|graph| graph.id.as_str());
        if crate::memory::lifecycle::is_forgotten(&root, &relative, candidate_id)? {
            return Err(MemoryError::ConfigError(format!(
                "cannot recreate forgotten note identity or path: {relative}"
            )));
        }
        let text = match markdown {
            Some(text) if text.trim().is_empty() => {
                return Err(MemoryError::ConfigError(
                    "rendered note must not be empty".into(),
                ));
            }
            Some(text) if text.len() > MAX_BODY_BYTES => {
                return Err(MemoryError::ConfigError(format!(
                    "rendered note is larger than {MAX_BODY_BYTES} bytes"
                )));
            }
            Some(text) => text.to_owned(),
            None => render(note, Utc::now().date_naive()),
        };
        if crate::memory::lifecycle::content_is_forgotten(&root, &text)? {
            return Err(MemoryError::ConfigError(format!(
                "cannot recreate forgotten note content at: {relative}"
            )));
        }
        // Publish through a same-directory temporary file. On Linux, every step from here is
        // performed relative to a single O_NOFOLLOW directory descriptor (`DirHandle`) opened
        // once against `dir`: this closes the window between the `canonicalize` check above and
        // the actual write/link/unlink, where a path-based re-open could otherwise follow a
        // symlink swapped in after the check. `hard_link`/`linkat` is used for admission: unlike
        // rename, it fails when the destination already exists and therefore cannot overwrite a
        // note created by another writer. sync_all makes the complete note durable before it
        // becomes visible to indexers.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_name = format!(".{}.tmp-{}-{}", slugify(title), std::process::id(), nonce);
        let dest_name = format!("{}.md", slugify(title));
        let tmp = dir.join(&tmp_name);
        let result = (|| -> std::io::Result<()> {
            let handle = crate::memory::fs_safety::DirHandle::open_nofollow(&dir)?;
            let mut file = handle.create_new_file(Path::new(&tmp_name))?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            // Test-only fault injection: widen the window between the temp file becoming
            // visible on disk and its publication (hard_link + unlink) so a crash-recovery
            // test's SIGKILL can deterministically land inside it. Gated behind an env var that
            // is never set in production, so this has zero effect outside tests.
            #[cfg(debug_assertions)]
            if std::env::var_os("PLAESY_MEMORY_TEST_SLOW_PUBLISH").is_some() {
                // Readiness signal for crash_recovery_test: the temp file is complete and the
                // crash window is open. Diagnostics go to stderr; stdout is protocol in JSON mode.
                eprintln!("plaesy-test: publish window open");
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            handle.link_within(Path::new(&tmp_name), Path::new(&dest_name))?;
            let _ = handle.remove_file(Path::new(&tmp_name));
            let _ = handle.sync();
            Ok(())
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(&tmp);
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                // Idempotency: a retried `remember` call (e.g. after a client-side timeout on a
                // call that actually succeeded) must not be treated as a conflict when the note
                // already on disk is exactly the one this call would have written. Compare
                // everything except `last_verified`, which legitimately differs by the day.
                if let Ok(existing) = std::fs::read_to_string(&path) {
                    if strip_last_verified(&existing) == strip_last_verified(&text) {
                        return Ok(path);
                    }
                }
                return Err(MemoryError::ConfigError(format!(
                    "note already exists with different content: {}",
                    path.display()
                )));
            }
            return Err(MemoryError::IoError {
                path: path.display().to_string(),
                source: e,
            });
        }
        // The outbox is durable before indexing starts. If the process dies after publication
        // (or during SQLite/HNSW replacement), startup reconciliation replays this path.
        self.database().enqueue_index(&relative).await?;
        self.index_file_at(&root, &path, None).await?;
        self.database().dequeue_index(&relative).await?;
        Ok(path)
    }
}

/// Remove the `last_verified: YYYY-MM-DD` line so two renders of the same note on different
/// days still compare equal for idempotency purposes.
pub(super) fn strip_last_verified(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("last_verified:"))
        .collect::<Vec<_>>()
        .join("\n")
}
