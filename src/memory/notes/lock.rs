//! `lock` helpers split out of `mod.rs`.

use super::*;

/// A best-effort cross-process lock for the remember write path.
///
/// The graph preflight and file admission must be serialized: separate processes have separate
/// SQLite connections and can otherwise both pass the duplicate-id check. A lock directory is
/// created atomically by the OS, so it also works when several CLI processes race.
pub(crate) struct WriteLock {
    path: PathBuf,
    stop: Arc<AtomicBool>,
    heartbeat: Option<JoinHandle<()>>,
}

impl WriteLock {
    /// Whether a live memory daemon currently owns this vault's write boundary.
    ///
    /// The daemon refreshes the lock directory every second. Match `acquire`'s ten-minute stale
    /// threshold so read-only commands can use its continuously maintained index without trying
    /// to become a competing writer.
    pub(crate) fn daemon_is_active(root: &Path) -> bool {
        let daemon = root.join(".plaesy-memory-daemon.lock");
        let Ok(meta) = std::fs::symlink_metadata(&daemon) else {
            return false;
        };
        if !meta.file_type().is_dir() {
            return false;
        }
        !meta
            .modified()
            .ok()
            .and_then(|time| time.elapsed().ok())
            .is_some_and(|age| age > Duration::from_secs(600))
    }

    pub(crate) fn acquire(root: &Path) -> Result<Self> {
        let daemon = root.join(".plaesy-memory-daemon.lock");
        if let Ok(meta) = std::fs::symlink_metadata(&daemon) {
            if !meta.file_type().is_dir() {
                return Err(MemoryError::ConfigError(format!(
                    "daemon lock path is not a directory: {}",
                    daemon.display()
                )));
            }
            let stale = meta
                .modified()
                .ok()
                .and_then(|time| time.elapsed().ok())
                .map(|age| age > Duration::from_secs(600))
                .unwrap_or(false);
            if stale {
                // The daemon heartbeat stopped, so permit recovery after a crash. An active
                // daemon refreshes this directory every second and cannot be reclaimed here.
                let _ = std::fs::remove_dir(&daemon);
            } else {
                return Err(MemoryError::ConfigError(
                    "memory daemon owns this vault; use its service API for writes".into(),
                ));
            }
        }
        Self::acquire_named(root, ".plaesy-memory-writer.lock")
    }

    /// Acquire a cross-process lock with a role-specific path.
    ///
    /// Retries for up to ~30s with capped exponential backoff: a `remember` call that holds the
    /// lock for its whole embed+index duration (roughly 1-2s) must not starve sibling agents
    /// whose turn comes after 5+ others ahead of them in the queue.
    pub(crate) fn acquire_named(root: &Path, name: &str) -> Result<Self> {
        let path = root.join(name);
        let mut wait = Duration::from_millis(10);
        let max_wait = Duration::from_millis(250);
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            match std::fs::create_dir(&path) {
                Ok(()) => {
                    let stop = Arc::new(AtomicBool::new(false));
                    let heartbeat_stop = Arc::clone(&stop);
                    let heartbeat_path = path.clone();
                    let heartbeat = std::thread::spawn(move || {
                        while !heartbeat_stop.load(Ordering::Relaxed) {
                            std::thread::sleep(Duration::from_secs(1));
                            if heartbeat_stop.load(Ordering::Relaxed) {
                                break;
                            }
                            // Create/remove a child so the lock directory mtime advances.
                            // An active writer must never be reclaimed as stale.
                            let heartbeat_file = heartbeat_path.join("heartbeat");
                            if std::fs::OpenOptions::new()
                                .create_new(true)
                                .write(true)
                                .open(&heartbeat_file)
                                .is_ok()
                            {
                                let _ = std::fs::remove_file(heartbeat_file);
                            }
                        }
                    });
                    return Ok(Self {
                        path,
                        stop,
                        heartbeat: Some(heartbeat),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Never follow a pre-existing symlink at the lock path. Apart from being
                    // ambiguous, it cannot be reclaimed with remove_dir and would create a
                    // permanent denial of service.
                    let lock_meta =
                        std::fs::symlink_metadata(&path).map_err(|error| MemoryError::IoError {
                            path: path.display().to_string(),
                            source: error,
                        })?;
                    if !lock_meta.file_type().is_dir() {
                        return Err(MemoryError::ConfigError(format!(
                            "lock path is not a directory: {}",
                            path.display()
                        )));
                    }
                    // A crashed writer must not permanently disable memory. Only reclaim a lock
                    // whose directory has been untouched for ten minutes.
                    let stale = lock_meta
                        .modified()
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .map(|age| age > Duration::from_secs(600))
                        .unwrap_or(false);
                    if stale {
                        let _ = std::fs::remove_dir(&path);
                        continue;
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(MemoryError::ConfigError(
                            "another memory write is still in progress".into(),
                        ));
                    }
                    std::thread::sleep(wait);
                    wait = std::cmp::min(wait * 2, max_wait);
                }
                Err(e) => {
                    return Err(MemoryError::IoError {
                        path: path.display().to_string(),
                        source: e,
                    });
                }
            }
        }
    }
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.heartbeat.take() {
            let _ = handle.join();
        }
        let _ = std::fs::remove_file(self.path.join("heartbeat"));
        let _ = std::fs::remove_dir(&self.path);
    }
}
