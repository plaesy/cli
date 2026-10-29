//! Generic, cached JSON configuration with atomic saves and hot reload.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use notify::event::{CreateKind, ModifyKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::ConfigError;

/// Path used when none is given: `.plaesy/config.json`, relative to the working directory.
#[must_use]
pub fn unified_config_path() -> PathBuf {
    PathBuf::from(".plaesy").join("config.json")
}

/// Quiet period after a change before the file is reloaded.
const DEBOUNCE: Duration = Duration::from_millis(100);

type Callback<T> = Arc<dyn Fn(&T) + Send + Sync>;

struct Shared<T> {
    path: PathBuf,
    default: Arc<T>,
    cache: RwLock<Option<Arc<T>>>,
    callbacks: Mutex<Vec<Callback<T>>>,
}

/// Holds one configuration file and its parsed, cached value.
///
/// `load` returns the cache when present and otherwise reads the file. A
/// missing file yields the default and is not an error. `save` writes to a
/// temporary file and renames it over the target, so readers never see a
/// partial file.
pub struct ConfigManager<T> {
    shared: Arc<Shared<T>>,
    watcher: Mutex<Option<WatchGuard>>,
}

impl<T> ConfigManager<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// Manages `path`, returning `default` while the file does not exist.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, default: T) -> Self {
        Self {
            shared: Arc::new(Shared {
                path: path.into(),
                default: Arc::new(default),
                cache: RwLock::new(None),
                callbacks: Mutex::new(Vec::new()),
            }),
            watcher: Mutex::new(None),
        }
    }

    /// Manages [`unified_config_path`].
    #[must_use]
    pub fn unified(default: T) -> Self {
        Self::new(unified_config_path(), default)
    }

    /// The managed file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.shared.path
    }

    /// Returns the configuration, reading the file on first use.
    ///
    /// # Errors
    ///
    /// Read failures, or JSON that does not match `T`.
    pub fn load(&self) -> Result<Arc<T>, ConfigError> {
        if let Some(cached) = self.get() {
            return Ok(cached);
        }
        load_from_disk(&self.shared)
    }

    /// The cached value, or `None` if nothing has been loaded or saved yet.
    #[must_use]
    pub fn get(&self) -> Option<Arc<T>> {
        self.shared
            .cache
            .read()
            .expect("config cache poisoned")
            .clone()
    }

    /// Writes `cfg` atomically, updates the cache, and notifies watchers.
    ///
    /// # Errors
    ///
    /// Serialization or filesystem failures. The previous file is kept on error.
    pub fn save(&self, cfg: T) -> Result<Arc<T>, ConfigError> {
        let cfg = store(&self.shared, cfg)?;
        notify_watchers(&self.shared, &cfg);
        Ok(cfg)
    }

    /// Registers `callback` to run with the new value on every change, and
    /// starts the file watcher on first use. Callbacks run on the watcher thread
    /// or on the thread calling `save`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Io`] when the watcher cannot be created or attached.
    pub fn watch(&self, callback: impl Fn(&T) + Send + Sync + 'static) -> Result<(), ConfigError> {
        self.shared
            .callbacks
            .lock()
            .expect("callbacks poisoned")
            .push(Arc::new(callback));
        let mut watcher = self.watcher.lock().expect("watcher poisoned");
        if watcher.is_none() {
            *watcher = Some(WatchGuard::start(Arc::clone(&self.shared))?);
        }
        Ok(())
    }

    /// Stops the file watcher. Registered callbacks are kept, and `watch`
    /// restarts the watcher.
    pub fn stop_watch(&self) {
        *self.watcher.lock().expect("watcher poisoned") = None;
    }

    /// Imports a configuration from `old_path` using `migrate`, saves it to this
    /// manager's path, and removes the old file. Does nothing if `old_path` is absent.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Migration`] when `migrate` rejects the old contents, or I/O errors.
    pub fn migrate_from<E>(
        &self,
        old_path: &Path,
        migrate: impl FnOnce(&[u8]) -> Result<T, E>,
    ) -> Result<(), ConfigError>
    where
        E: std::fmt::Display,
    {
        let bytes = match fs::read(old_path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(ConfigError::io(
                    format!("reading {}", old_path.display()),
                    e,
                ))
            }
        };
        let cfg = migrate(&bytes).map_err(|e| ConfigError::Migration(e.to_string()))?;
        store(&self.shared, cfg)?;
        match fs::remove_file(old_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ConfigError::io(
                format!("removing {}", old_path.display()),
                e,
            )),
        }
    }
}

fn load_from_disk<T>(shared: &Shared<T>) -> Result<Arc<T>, ConfigError>
where
    T: DeserializeOwned,
{
    let bytes = match fs::read(&shared.path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let default = Arc::clone(&shared.default);
            *shared.cache.write().expect("config cache poisoned") = Some(Arc::clone(&default));
            return Ok(default);
        }
        Err(e) => {
            return Err(ConfigError::io(
                format!("reading config {}", shared.path.display()),
                e,
            ))
        }
    };
    let cfg = Arc::new(serde_json::from_slice::<T>(&bytes)?);
    *shared.cache.write().expect("config cache poisoned") = Some(Arc::clone(&cfg));
    Ok(cfg)
}

/// Serializes and writes `cfg`, then caches it. Does not notify watchers.
fn store<T>(shared: &Shared<T>, cfg: T) -> Result<Arc<T>, ConfigError>
where
    T: Serialize,
{
    let json = serde_json::to_string_pretty(&cfg)?;
    if let Some(parent) = shared.path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|e| ConfigError::io(format!("creating {}", parent.display()), e))?;
    }
    write_atomic(&shared.path, json.as_bytes())?;
    let cfg = Arc::new(cfg);
    *shared.cache.write().expect("config cache poisoned") = Some(Arc::clone(&cfg));
    Ok(cfg)
}

/// Writes `data` to `<path>.tmp`, then renames it over `path`.
fn write_atomic(path: &Path, data: &[u8]) -> Result<(), ConfigError> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, data).map_err(|e| ConfigError::io(format!("writing {}", tmp.display()), e))?;
    fs::rename(&tmp, path)
        .map_err(|e| ConfigError::io(format!("renaming to {}", path.display()), e))
}

/// Calls every callback with `cfg`. Callbacks are copied first, so a callback may
/// register another watcher without deadlocking.
fn notify_watchers<T>(shared: &Shared<T>, cfg: &T) {
    let callbacks: Vec<Callback<T>> = shared.callbacks.lock().expect("callbacks poisoned").clone();
    for callback in callbacks {
        callback(cfg);
    }
}

/// Owns the OS watcher. Dropping it stops the watcher, which closes the event
/// channel and ends the reload thread.
struct WatchGuard {
    _watcher: RecommendedWatcher,
}

impl WatchGuard {
    fn start<T>(shared: Arc<Shared<T>>) -> Result<Self, ConfigError>
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            // The receiver is gone only after the guard is dropped, so a failed
            // send means nothing is listening.
            let _ = tx.send(event);
        })
        .map_err(|e| ConfigError::io("creating file watcher", io::Error::other(e)))?;

        let dir = shared
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .map_err(|e| {
                ConfigError::io(format!("watching {}", dir.display()), io::Error::other(e))
            })?;

        thread::spawn(move || reload_loop(&shared, &rx));
        Ok(Self { _watcher: watcher })
    }
}

fn reload_loop<T>(shared: &Shared<T>, rx: &mpsc::Receiver<notify::Result<notify::Event>>)
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    // Far enough away to mean "no pending change" without overflowing Instant.
    const IDLE: Duration = Duration::from_secs(3600);
    let mut deadline: Option<Instant> = None;
    loop {
        let timeout = deadline.map_or(IDLE, |d| d.saturating_duration_since(Instant::now()));
        match rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                if is_write_to(&event, &shared.path) {
                    deadline = Some(Instant::now() + DEBOUNCE);
                }
            }
            // A watcher error does not stop watching. The next event still counts.
            Ok(Err(_)) => {}
            Err(RecvTimeoutError::Timeout) => {
                if deadline.take().is_some() {
                    if let Ok(cfg) = load_from_disk(shared) {
                        notify_watchers(shared, &cfg);
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Reports whether `event` creates or modifies the managed file.
fn is_write_to(event: &notify::Event, path: &Path) -> bool {
    let relevant = matches!(
        event.kind,
        EventKind::Create(CreateKind::File | CreateKind::Any)
            | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any)
    );
    let name = path.file_name();
    relevant && event.paths.iter().any(|p| p.file_name() == name)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        count: i64,
    }

    fn sample(name: &str, count: i64) -> Sample {
        Sample {
            name: name.into(),
            count,
        }
    }

    #[test]
    fn missing_file_yields_the_default_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = ConfigManager::new(dir.path().join("c.json"), sample("def", 1));
        assert_eq!(*mgr.load().unwrap(), sample("def", 1));
        assert!(!dir.path().join("c.json").exists());
    }

    #[test]
    fn save_then_load_round_trips_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("c.json");
        let mgr = ConfigManager::new(&path, sample("def", 0));
        mgr.save(sample("saved", 7)).unwrap();

        let fresh = ConfigManager::new(&path, sample("def", 0));
        assert_eq!(*fresh.load().unwrap(), sample("saved", 7));
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn invalid_json_is_an_error_not_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        fs::write(&path, "{ nope").unwrap();
        let mgr = ConfigManager::new(&path, sample("def", 0));
        assert!(matches!(mgr.load(), Err(ConfigError::Json(_))));
    }

    #[test]
    fn migrate_moves_contents_and_removes_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.json");
        let new = dir.path().join("new.json");
        fs::write(&old, r#"{"name":"legacy","count":3}"#).unwrap();

        let mgr = ConfigManager::new(&new, sample("def", 0));
        mgr.migrate_from(&old, |bytes| serde_json::from_slice::<Sample>(bytes))
            .unwrap();

        assert!(!old.exists());
        assert_eq!(*mgr.load().unwrap(), sample("legacy", 3));
    }

    #[test]
    fn migrate_with_no_old_file_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = ConfigManager::new(dir.path().join("new.json"), sample("def", 0));
        mgr.migrate_from(&dir.path().join("absent.json"), |_: &[u8]| {
            Ok::<_, String>(sample("x", 1))
        })
        .unwrap();
        assert!(mgr.get().is_none());
    }

    #[test]
    fn save_notifies_registered_callbacks() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = ConfigManager::new(dir.path().join("c.json"), sample("def", 0));
        let (tx, rx) = channel();
        mgr.watch(move |cfg: &Sample| {
            let _ = tx.send(cfg.clone());
        })
        .unwrap();
        mgr.save(sample("changed", 2)).unwrap();
        let seen = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("callback ran");
        assert_eq!(seen, sample("changed", 2));
        mgr.stop_watch();
    }

    #[test]
    fn external_edit_triggers_a_reload_for_watchers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        let mgr = ConfigManager::new(&path, sample("def", 0));
        let (tx, rx) = channel();
        mgr.watch(move |cfg: &Sample| {
            let _ = tx.send(cfg.clone());
        })
        .unwrap();

        // Give the watcher a moment to attach before the edit.
        thread::sleep(Duration::from_millis(200));
        fs::write(&path, r#"{"name":"edited","count":9}"#).unwrap();

        let seen = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("watcher reported the external edit");
        assert_eq!(seen, sample("edited", 9));
        mgr.stop_watch();
    }
}
