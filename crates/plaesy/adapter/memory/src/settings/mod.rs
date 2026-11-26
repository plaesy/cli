//! User settings written by `plaesy onboard`: `<root>/.plaesy/config.toml`.
//!
//! The root defaults to the user's home directory, the same on Windows, Linux, and macOS, like
//! `~/.claude` and `~/.hermes`. Set `PLAESY_ROOT` to use another root. The file has the same
//! schema as the project file `.plaesy/config.toml`
//! (see [`crate::memory::config::MemoryConfig`]). It sits below `.plaesy/config.toml` and
//! `MEMORY_*` variables in the precedence order of [`crate::memory::config::MemoryConfig::load`].

use crate::memory::config::MemoryConfig;
use crate::memory::error::MemoryError;
use crate::memory::Result;
use std::path::{Path, PathBuf};

/// Current settings format version.
pub const SETTINGS_VERSION: u32 = 1;

/// Folder name under the root that holds `config.toml`.
pub const SETTINGS_DIR: &str = ".plaesy";

/// The fields that onboarding writes. Every field is optional; absent means "use the default".
///
/// On disk these map to the nested keys of `config.toml`: `watch_dir`, `database.db_path`,
/// `embeddings.model_name`, `search.rerank`, and `search.exclude_dirs`.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub version: u32,
    /// Folder of notes to index (`watch_dir`).
    pub notes_dir: Option<PathBuf>,
    /// SQLite database file (`database.db_path`).
    pub db_path: Option<PathBuf>,
    /// Embedding model folder name, for example `all-MiniLM-L6-v2` (`embeddings.model_name`).
    pub model_name: Option<String>,
    /// Cross-encoder reranking on or off (`search.rerank`).
    pub rerank: Option<bool>,
    /// Folder or file names kept out of the index (`search.exclude_dirs`).
    pub exclude_dirs: Option<Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            notes_dir: None,
            db_path: None,
            model_name: None,
            rerank: None,
            exclude_dirs: None,
        }
    }
}

impl Settings {
    /// Copy the fields that are set onto `cfg`. Unset fields leave `cfg` unchanged.
    pub fn apply_to(&self, cfg: &mut MemoryConfig) {
        if let Some(v) = &self.notes_dir {
            cfg.watch_dir = v.clone();
        }
        if let Some(v) = &self.db_path {
            cfg.database.db_path = v.clone();
        }
        if let Some(v) = &self.model_name {
            cfg.embeddings.model_name = v.clone();
        }
        if let Some(v) = self.rerank {
            cfg.search.rerank = v;
        }
        if let Some(v) = &self.exclude_dirs {
            cfg.search.exclude_dirs = v.clone();
        }
    }

    fn from_json(value: &serde_json::Value, path: &Path) -> Result<Self> {
        let field = |pointer: &str| -> Result<Option<serde_json::Value>> {
            Ok(value.pointer(pointer).filter(|v| !v.is_null()).cloned())
        };
        let invalid = |e: serde_json::Error| {
            MemoryError::ConfigError(format!("invalid settings {}: {e}", path.display()))
        };
        Ok(Settings {
            version: match field("/version")? {
                Some(v) => serde_json::from_value(v).map_err(invalid)?,
                None => SETTINGS_VERSION,
            },
            notes_dir: field("/watch_dir")?
                .map(serde_json::from_value)
                .transpose()
                .map_err(invalid)?,
            db_path: field("/database/db_path")?
                .map(serde_json::from_value)
                .transpose()
                .map_err(invalid)?,
            model_name: field("/embeddings/model_name")?
                .map(serde_json::from_value)
                .transpose()
                .map_err(invalid)?,
            rerank: field("/search/rerank")?
                .map(serde_json::from_value)
                .transpose()
                .map_err(invalid)?,
            exclude_dirs: field("/search/exclude_dirs")?
                .map(serde_json::from_value)
                .transpose()
                .map_err(invalid)?,
        })
    }

    fn to_json(&self) -> serde_json::Value {
        use serde_json::{json, Map, Value};
        let mut root = Map::new();
        root.insert("version".into(), json!(self.version));
        if let Some(v) = &self.notes_dir {
            root.insert("watch_dir".into(), json!(v));
        }
        let mut database = Map::new();
        if let Some(v) = &self.db_path {
            database.insert("db_path".into(), json!(v));
        }
        let mut embeddings = Map::new();
        if let Some(v) = &self.model_name {
            embeddings.insert("model_name".into(), json!(v));
        }
        let mut search = Map::new();
        if let Some(v) = self.rerank {
            search.insert("rerank".into(), json!(v));
        }
        if let Some(v) = &self.exclude_dirs {
            search.insert("exclude_dirs".into(), json!(v));
        }
        for (key, section) in [
            ("database", database),
            ("embeddings", embeddings),
            ("search", search),
        ] {
            if !section.is_empty() {
                root.insert(key.into(), Value::Object(section));
            }
        }
        Value::Object(root)
    }
}

/// Settings file location: `<root>/.plaesy/config.toml`, where `<root>` is `PLAESY_ROOT` or the home directory.
pub fn default_path() -> Option<PathBuf> {
    root_from(
        std::env::var_os("PLAESY_ROOT").map(PathBuf::from),
        dirs::home_dir(),
    )
    .map(|root| file_in_root(&root))
}

/// Default root for `plaesy onboard`: `PLAESY_ROOT`, else the home directory.
pub fn default_root() -> Option<PathBuf> {
    root_from(
        std::env::var_os("PLAESY_ROOT").map(PathBuf::from),
        dirs::home_dir(),
    )
}

/// Settings file location for an explicit root.
pub fn file_in_root(root: &Path) -> PathBuf {
    root.join(SETTINGS_DIR).join("config.toml")
}

fn root_from(env_root: Option<PathBuf>, platform: Option<PathBuf>) -> Option<PathBuf> {
    env_root.filter(|p| !p.as_os_str().is_empty()).or(platform)
}

/// Read settings from `path`. Returns `Ok(None)` when the file does not exist.
///
/// # Errors
///
/// - `ConfigError` if the file exists but is not valid TOML
pub fn load(path: &Path) -> Result<Option<Settings>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(MemoryError::ConfigError(format!(
                "cannot inspect {}: {error}",
                path.display()
            )));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(MemoryError::ConfigError(format!(
            "settings path is not a regular file: {}",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| MemoryError::ConfigError(format!("cannot read {}: {e}", path.display())))?;
    let table: toml::Table = toml::from_str(&text).map_err(|e| {
        MemoryError::ConfigError(format!("invalid settings {}: {e}", path.display()))
    })?;
    let value = serde_json::to_value(table)
        .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
    Settings::from_json(&value, path).map(Some)
}

/// Write settings to `path`, creating parent folders. Overwrites an existing file.
///
/// # Errors
///
/// - `ConfigError` if the folder cannot be created or the file cannot be written
pub fn save(path: &Path, settings: &Settings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            MemoryError::ConfigError(format!("cannot create {}: {e}", parent.display()))
        })?;
    }
    let text = toml::to_string_pretty(&settings.to_json())
        .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            MemoryError::ConfigError(format!("invalid settings path: {}", path.display()))
        })?;
    let tmp = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all((text + "\n").as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| MemoryError::ConfigError(format!("cannot write {}: {e}", path.display())))
}

pub mod env_file;

#[cfg(test)]
#[path = "settings_test.rs"]
mod settings_test;
