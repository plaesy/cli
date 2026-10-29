//! Response caches: an in-memory LRU with TTL, a file cache, and a tiered cache of both.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::types::{BackendResult, Platform};

/// Cache counters.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CacheStats {
    pub entries: i64,
    pub hits: i64,
    pub misses: i64,
    pub hit_rate: f64,
    pub size_bytes: i64,
}

impl CacheStats {
    fn with_rate(mut self) -> Self {
        if self.hits + self.misses > 0 {
            self.hit_rate = self.hits as f64 / (self.hits + self.misses) as f64;
        }
        self
    }
}

/// Storage for backend results.
pub trait CacheBackend: Send + Sync {
    fn get(&self, key: &str) -> Option<BackendResult>;

    /// Stores `value`. A zero `ttl` uses the cache's default.
    ///
    /// # Errors
    ///
    /// The value cannot be stored (file caches only).
    fn set(&self, key: &str, value: &BackendResult, ttl: Duration) -> io::Result<()>;

    /// # Errors
    ///
    /// The entry exists but cannot be removed (file caches only).
    fn delete(&self, key: &str) -> io::Result<()>;

    /// # Errors
    ///
    /// The cache cannot be emptied (file caches only).
    fn clear(&self) -> io::Result<()>;

    fn stats(&self) -> CacheStats;
}

struct MemoryEntry {
    value: BackendResult,
    created: Instant,
    expires: Instant,
    hits: i64,
}

#[derive(Default)]
struct MemoryState {
    entries: HashMap<String, MemoryEntry>,
    stats: CacheStats,
}

/// An in-memory cache with TTL. When full, the oldest entry is evicted.
///
/// Expired entries are removed when they are read, and before an eviction is needed. There is no
/// background sweep thread.
pub struct MemoryCache {
    state: Mutex<MemoryState>,
    max_size: usize,
    default_ttl: Duration,
}

impl MemoryCache {
    #[must_use]
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            state: Mutex::new(MemoryState::default()),
            max_size,
            default_ttl,
        }
    }

    fn evict_one(state: &mut MemoryState) {
        let now = Instant::now();
        let expired: Vec<String> = state
            .entries
            .iter()
            .filter(|(_, e)| now > e.expires)
            .map(|(k, _)| k.clone())
            .collect();
        for key in expired {
            state.entries.remove(&key);
            state.stats.entries -= 1;
        }
        if let Some(oldest) = state
            .entries
            .iter()
            .min_by_key(|(_, e)| e.created)
            .map(|(k, _)| k.clone())
        {
            state.entries.remove(&oldest);
            state.stats.entries -= 1;
        }
    }
}

impl CacheBackend for MemoryCache {
    fn get(&self, key: &str) -> Option<BackendResult> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let now = Instant::now();
        match state.entries.get(key) {
            None => {
                state.stats.misses += 1;
                None
            }
            Some(entry) if now > entry.expires => {
                state.entries.remove(key);
                state.stats.entries -= 1;
                state.stats.misses += 1;
                None
            }
            Some(_) => {
                state.stats.hits += 1;
                let entry = state.entries.get_mut(key)?;
                entry.hits += 1;
                Some(entry.value.clone())
            }
        }
    }

    fn set(&self, key: &str, value: &BackendResult, ttl: Duration) -> io::Result<()> {
        let ttl = if ttl.is_zero() { self.default_ttl } else { ttl };
        let now = Instant::now();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.entries.contains_key(key) {
            if self.max_size > 0 && state.entries.len() >= self.max_size {
                Self::evict_one(&mut state);
            }
            state.stats.entries += 1;
        }
        state.entries.insert(
            key.to_owned(),
            MemoryEntry {
                value: value.clone(),
                created: now,
                expires: now + ttl,
                hits: 0,
            },
        );
        Ok(())
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.entries.remove(key).is_some() {
            state.stats.entries -= 1;
        }
        Ok(())
    }

    fn clear(&self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *state = MemoryState::default();
        Ok(())
    }

    fn stats(&self) -> CacheStats {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.stats.with_rate()
    }
}

/// The on-disk form of one cached result.
#[derive(Debug, Serialize, Deserialize)]
struct StoredEntry {
    key: String,
    backend: String,
    platform: String,
    success: bool,
    data: String,
    error: String,
    latency_ns: u64,
    timestamp: DateTime<Utc>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    hits: i64,
}

impl StoredEntry {
    fn to_result(&self) -> Option<BackendResult> {
        let platform: Platform = self.platform.parse().ok()?;
        Some(BackendResult {
            backend: self.backend.clone(),
            platform,
            success: self.success,
            data: self.data.clone(),
            error: self.error.clone(),
            latency: Duration::from_nanos(self.latency_ns),
            timestamp: self.timestamp,
        })
    }
}

/// A cache with one JSON file per entry, named by the hash of its key.
pub struct FileCache {
    dir: PathBuf,
    max_size: u64,
    default_ttl: Duration,
    stats: Mutex<CacheStats>,
}

impl FileCache {
    /// Opens (and creates) the cache folder `dir`. A `max_size_mb` of zero means no size limit.
    ///
    /// # Errors
    ///
    /// The folder cannot be created.
    pub fn new(
        dir: impl Into<PathBuf>,
        max_size_mb: i64,
        default_ttl: Duration,
    ) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            max_size: u64::try_from(max_size_mb.max(0)).unwrap_or(0) * 1024 * 1024,
            default_ttl,
            stats: Mutex::new(CacheStats::default()),
        })
    }

    fn path_for(&self, key: &str) -> PathBuf {
        let digest = Sha256::digest(key.as_bytes());
        let name: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
        self.dir.join(format!("{name}.cache"))
    }

    fn lock_stats(&self) -> std::sync::MutexGuard<'_, CacheStats> {
        self.stats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Removes the oldest cache files until the folder is under the size limit. `keep` stays.
    fn enforce_size(&self, keep: &Path) {
        if self.max_size == 0 {
            return;
        }
        let Ok(read) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
        let mut total = 0u64;
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "cache") {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            total += meta.len();
            files.push((
                path,
                meta.len(),
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            ));
        }
        if total <= self.max_size {
            return;
        }
        files.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, _) in files {
            if total <= self.max_size {
                break;
            }
            if path == keep {
                continue;
            }
            if fs::remove_file(&path).is_ok() {
                total -= size;
                let mut stats = self.lock_stats();
                if stats.entries > 0 {
                    stats.entries -= 1;
                }
            }
        }
    }
}

impl CacheBackend for FileCache {
    fn get(&self, key: &str) -> Option<BackendResult> {
        let path = self.path_for(key);
        let miss = |this: &Self| {
            this.lock_stats().misses += 1;
            None
        };
        let Ok(data) = fs::read(&path) else {
            return miss(self);
        };
        let Ok(mut entry) = serde_json::from_slice::<StoredEntry>(&data) else {
            return miss(self);
        };
        if Utc::now() > entry.expires_at {
            let _ = fs::remove_file(&path);
            return miss(self);
        }
        let result = entry.to_result()?;
        self.lock_stats().hits += 1;
        entry.hits += 1;
        if let Ok(json) = serde_json::to_vec(&entry) {
            let _ = fs::write(&path, json);
        }
        Some(result)
    }

    fn set(&self, key: &str, value: &BackendResult, ttl: Duration) -> io::Result<()> {
        let ttl = if ttl.is_zero() { self.default_ttl } else { ttl };
        let now = Utc::now();
        let entry = StoredEntry {
            key: key.to_owned(),
            backend: value.backend.clone(),
            platform: value.platform.as_str().to_owned(),
            success: value.success,
            data: value.data.clone(),
            error: value.error.clone(),
            latency_ns: u64::try_from(value.latency.as_nanos()).unwrap_or(u64::MAX),
            timestamp: value.timestamp,
            created_at: now,
            expires_at: now + chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::MAX),
            hits: 0,
        };
        let json = serde_json::to_vec(&entry).map_err(io::Error::other)?;
        let path = self.path_for(key);
        let is_new = !path.exists();
        fs::write(&path, json)?;
        {
            let mut stats = self.lock_stats();
            if is_new {
                stats.entries += 1;
            }
        }
        self.enforce_size(&path);
        Ok(())
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        if fs::remove_file(self.path_for(key)).is_ok() {
            let mut stats = self.lock_stats();
            stats.entries -= 1;
        }
        Ok(())
    }

    fn clear(&self) -> io::Result<()> {
        if let Ok(read) = fs::read_dir(&self.dir) {
            for entry in read.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
        *self.lock_stats() = CacheStats::default();
        Ok(())
    }

    fn stats(&self) -> CacheStats {
        (*self.lock_stats()).with_rate()
    }
}

/// Memory in front of a file cache. A file hit is promoted into memory.
pub struct TieredCache {
    memory: MemoryCache,
    file: FileCache,
}

impl TieredCache {
    /// # Errors
    ///
    /// The file cache folder cannot be created.
    pub fn new(
        memory_size: usize,
        file_dir: impl Into<PathBuf>,
        file_size_mb: i64,
        ttl: Duration,
    ) -> io::Result<Self> {
        Ok(Self {
            memory: MemoryCache::new(memory_size, ttl),
            file: FileCache::new(file_dir, file_size_mb, ttl)?,
        })
    }
}

impl CacheBackend for TieredCache {
    fn get(&self, key: &str) -> Option<BackendResult> {
        if let Some(value) = self.memory.get(key) {
            return Some(value);
        }
        let value = self.file.get(key)?;
        let _ = self.memory.set(key, &value, self.file.default_ttl);
        Some(value)
    }

    fn set(&self, key: &str, value: &BackendResult, ttl: Duration) -> io::Result<()> {
        self.memory.set(key, value, ttl)?;
        self.file.set(key, value, ttl)
    }

    fn delete(&self, key: &str) -> io::Result<()> {
        self.memory.delete(key)?;
        self.file.delete(key)
    }

    fn clear(&self) -> io::Result<()> {
        self.memory.clear()?;
        self.file.clear()
    }

    fn stats(&self) -> CacheStats {
        let mem = self.memory.stats();
        let file = self.file.stats();
        // A memory miss is not final: the file tier is asked next. Its misses are the lookups
        // that failed both tiers, and it holds every entry, so it is the source for `entries`.
        CacheStats {
            entries: file.entries,
            hits: mem.hits + file.hits,
            misses: file.misses,
            hit_rate: 0.0,
            size_bytes: mem.size_bytes + file.size_bytes,
        }
        .with_rate()
    }
}

/// The cache key for a query. Options are rendered in sorted order, as Go's `%v` renders maps.
#[must_use]
pub fn cache_key(
    platform: Platform,
    backend: &str,
    query: &str,
    opts: &super::types::Options,
) -> String {
    let rendered: Vec<String> = opts.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    let data = format!("{platform}|{backend}|{query}|map[{}]", rendered.join(" "));
    let digest = Sha256::digest(data.as_bytes());
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(data: &str) -> BackendResult {
        let mut r = BackendResult::new("test", Platform::Web);
        r.success = true;
        r.data = data.to_owned();
        r
    }

    #[test]
    fn memory_cache_returns_what_was_stored_and_counts_hits() {
        let cache = MemoryCache::new(10, Duration::from_secs(60));
        cache.set("k", &result("v"), Duration::ZERO).expect("set");
        assert_eq!(cache.get("k").map(|r| r.data), Some("v".to_owned()));
        assert!(cache.get("missing").is_none());
        let stats = cache.stats();
        assert_eq!((stats.hits, stats.misses, stats.entries), (1, 1, 1));
    }

    #[test]
    fn a_full_memory_cache_evicts_the_oldest_entry() {
        let cache = MemoryCache::new(1, Duration::from_secs(60));
        cache.set("a", &result("1"), Duration::ZERO).expect("set");
        cache.set("b", &result("2"), Duration::ZERO).expect("set");
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
    }

    #[test]
    fn file_cache_survives_a_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = FileCache::new(dir.path(), 10, Duration::from_secs(60)).expect("open");
        cache
            .set("k", &result("persisted"), Duration::ZERO)
            .expect("set");
        let reopened = FileCache::new(dir.path(), 10, Duration::from_secs(60)).expect("reopen");
        assert_eq!(
            reopened.get("k").map(|r| r.data),
            Some("persisted".to_owned())
        );
    }

    #[test]
    fn cache_keys_do_not_depend_on_option_insertion_order() {
        let mut a = super::super::types::Options::new();
        a.insert("x".into(), "1".into());
        a.insert("y".into(), "2".into());
        let mut b = super::super::types::Options::new();
        b.insert("y".into(), "2".into());
        b.insert("x".into(), "1".into());
        assert_eq!(
            cache_key(Platform::Web, "", "q", &a),
            cache_key(Platform::Web, "", "q", &b)
        );
    }
}
