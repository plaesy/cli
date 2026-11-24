//! Explicit per-user registry of Plaesy project roots.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

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
fn registry_file() -> Result<PathBuf, Error> {
    Ok(data_dir()?.join("projects.json"))
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
    let file = registry_file()?;
    with_lock(&data_dir()?, || f(&file))
}
fn read(file: &Path) -> Result<Vec<Project>, Error> {
    if !file.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&fs::read(file)?)?)
}
fn write(file: &Path, entries: &[Project]) -> Result<(), Error> {
    let parent = file.parent().ok_or_else(|| {
        Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "registry file has no parent directory",
        ))
    })?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".projects-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, serde_json::to_vec_pretty(entries)?)?;
    fs::rename(tmp, file)?;
    Ok(())
}
/// Add or refresh a project path in the per-user registry; repository content is never scanned.
pub fn register(path: &Path) -> Result<Project, Error> {
    let path = normalized(path)?;
    with_registry(|file| {
        let mut entries = read(file)?;
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
        let entries = read(file)?;
        let valid: Vec<_> = entries
            .into_iter()
            .filter(|p| normalized(&p.path).is_ok())
            .collect();
        if valid.len() != read(file)?.len() {
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
    list()?
        .into_iter()
        .find(|p| p.id == selector || p.path == Path::new(selector))
        .ok_or_else(|| Error::NotFound(selector.to_owned()))
}

#[cfg(test)]
#[path = "projects_test.rs"]
mod projects_test;
