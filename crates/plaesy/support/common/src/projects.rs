//! Explicit per-user registry of Plaesy project roots.
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

/// Registry file name inside the per-user data directory.
const REGISTRY_FILE: &str = "projects.json";
/// Per-project settings, keyed by project ID, stored beside the registry.
const CONFIG_FILE: &str = "project-config.json";
const MAX_KEY_BYTES: usize = 64;
const MAX_VALUE_BYTES: usize = 4096;

/// Settings of one project: key to value.
pub type Settings = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("project path does not contain a .plaesy directory: {0}")]
    NotProject(String),
    #[error("project not registered: {0}")]
    NotFound(String),
    #[error("invalid project setting: {0}")]
    InvalidSetting(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Per-user data directory shared by the project registry and the workspace groups.
pub(crate) fn data_dir() -> Result<PathBuf, Error> {
    let root = dirs::data_local_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "platform data directory unavailable",
        )
    })?;
    Ok(root.join("plaesy"))
}
fn normalized(path: &Path) -> Result<PathBuf, Error> {
    let p = fs::canonicalize(path)?;
    if !p.join(".plaesy").is_dir() {
        return Err(Error::NotProject(p.display().to_string()));
    }
    Ok(p)
}
/// Runs `f` while holding the lock that guards every per-user registry file in `dir`.
pub(crate) fn with_lock<T, E: From<Error>>(
    dir: &Path,
    f: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    fs::create_dir_all(dir).map_err(Error::from)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".projects.lock"))
        .map_err(Error::from)?;
    lock.lock().map_err(Error::from)?;
    let result = f();
    let _ = lock.unlock();
    result
}
fn with_registry<T>(f: impl FnOnce(&Path) -> Result<T, Error>) -> Result<T, Error> {
    let dir = data_dir()?;
    with_lock(&dir, || f(&dir.join(REGISTRY_FILE)))
}
/// Reads a JSON file; a missing file is the default value.
fn read<T: DeserializeOwned + Default>(file: &Path) -> Result<T, Error> {
    if !file.exists() {
        return Ok(T::default());
    }
    Ok(serde_json::from_slice(&fs::read(file)?)?)
}
/// Writes a JSON file through a temporary file and a rename, so a reader never sees a partial file.
fn write<T: Serialize + ?Sized>(file: &Path, value: &T) -> Result<(), Error> {
    let parent = file.parent().ok_or_else(|| {
        Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "registry file has no parent directory",
        ))
    })?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".projects-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(tmp, file)?;
    Ok(())
}
/// Finds an entry by stable ID, by its recorded path, or by a path that canonicalizes to it.
fn find<'a>(entries: &'a [Project], selector: &str) -> Option<&'a Project> {
    let canonical = fs::canonicalize(selector).ok();
    entries.iter().find(|p| {
        p.id == selector
            || p.path == Path::new(selector)
            || canonical.as_deref() == Some(p.path.as_path())
    })
}
/// Add or refresh a project path in the per-user registry; repository content is never scanned.
pub fn register(path: &Path) -> Result<Project, Error> {
    let path = normalized(path)?;
    with_registry(|file| {
        let mut entries: Vec<Project> = read(file)?;
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(path.to_string_lossy().as_bytes());
        let id = format!(
            "p-{}",
            digest
                .iter()
                .take(8)
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let project = Project { id, path };
        entries.retain(|p| p.id != project.id && p.path != project.path);
        entries.push(project.clone());
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        write(file, &entries)?;
        Ok(project)
    })
}
/// List registered projects, omitting roots that no longer exist or are no longer Plaesy projects.
pub fn list() -> Result<Vec<Project>, Error> {
    with_registry(|file| {
        let entries: Vec<Project> = read(file)?;
        let valid: Vec<Project> = entries
            .iter()
            .filter(|p| normalized(&p.path).is_ok())
            .cloned()
            .collect();
        if valid.len() != entries.len() {
            write(file, &valid)?;
        }
        Ok(valid)
    })
}
/// Whether `path` is currently a Plaesy project root. Unlike `list`, this never edits the registry.
#[must_use]
pub fn is_project_root(path: &Path) -> bool {
    normalized(path).is_ok()
}
/// Resolve a registered project by stable ID or canonical path.
pub fn resolve(selector: &str) -> Result<Project, Error> {
    let entries = list()?;
    find(&entries, selector)
        .cloned()
        .ok_or_else(|| Error::NotFound(selector.to_owned()))
}

/// Removes a project from the registry and drops its settings. Its files are never touched.
/// Unlike `resolve`, a stale entry (whose root no longer exists) can still be removed.
pub fn remove(selector: &str) -> Result<Project, Error> {
    remove_in(&data_dir()?, selector)
}
fn remove_in(dir: &Path, selector: &str) -> Result<Project, Error> {
    with_lock(dir, || {
        let registry = dir.join(REGISTRY_FILE);
        let mut entries: Vec<Project> = read(&registry)?;
        let project = find(&entries, selector)
            .cloned()
            .ok_or_else(|| Error::NotFound(selector.to_owned()))?;
        entries.retain(|p| p.id != project.id);
        write(&registry, &entries)?;
        let config_file = dir.join(CONFIG_FILE);
        let mut config: BTreeMap<String, Settings> = read(&config_file)?;
        if config.remove(&project.id).is_some() {
            write(&config_file, &config)?;
        }
        Ok(project)
    })
}

/// The project and its settings. A missing key set is an empty map.
pub fn settings(selector: &str) -> Result<(Project, Settings), Error> {
    settings_in(&data_dir()?, selector)
}
fn settings_in(dir: &Path, selector: &str) -> Result<(Project, Settings), Error> {
    with_lock(dir, || {
        let entries: Vec<Project> = read(&dir.join(REGISTRY_FILE))?;
        let project = find(&entries, selector)
            .cloned()
            .ok_or_else(|| Error::NotFound(selector.to_owned()))?;
        let mut config: BTreeMap<String, Settings> = read(&dir.join(CONFIG_FILE))?;
        let settings = config.remove(&project.id).unwrap_or_default();
        Ok((project, settings))
    })
}

/// Sets one setting of a project, or removes it when `value` is `None`. Returns the project.
/// Keys are 1-64 ASCII letters, digits, `.`, `-` or `_`; values are at most 4096 bytes with no
/// control characters.
pub fn set_setting(selector: &str, key: &str, value: Option<&str>) -> Result<Project, Error> {
    set_setting_in(&data_dir()?, selector, key, value)
}
fn set_setting_in(
    dir: &Path,
    selector: &str,
    key: &str,
    value: Option<&str>,
) -> Result<Project, Error> {
    validate_setting(key, value)?;
    with_lock(dir, || {
        let entries: Vec<Project> = read(&dir.join(REGISTRY_FILE))?;
        let project = find(&entries, selector)
            .cloned()
            .ok_or_else(|| Error::NotFound(selector.to_owned()))?;
        let config_file = dir.join(CONFIG_FILE);
        let mut config: BTreeMap<String, Settings> = read(&config_file)?;
        let settings = config.entry(project.id.clone()).or_default();
        match value {
            Some(value) => {
                settings.insert(key.to_owned(), value.to_owned());
            }
            None => {
                settings.remove(key);
            }
        }
        if settings.is_empty() {
            config.remove(&project.id);
        }
        write(&config_file, &config)?;
        Ok(project)
    })
}
fn validate_setting(key: &str, value: Option<&str>) -> Result<(), Error> {
    let key_ok = !key.is_empty()
        && key.len() <= MAX_KEY_BYTES
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    if !key_ok {
        return Err(Error::InvalidSetting(format!(
            "key must be 1-{MAX_KEY_BYTES} letters, digits, '.', '-' or '_': {key:?}"
        )));
    }
    if let Some(value) = value {
        if value.len() > MAX_VALUE_BYTES || value.chars().any(char::is_control) {
            return Err(Error::InvalidSetting(format!(
                "value must be at most {MAX_VALUE_BYTES} bytes with no control characters"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "projects_test.rs"]
mod projects_test;
