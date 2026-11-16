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

fn registry_file() -> Result<PathBuf, Error> {
    let root = dirs::data_local_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "platform data directory unavailable",
        )
    })?;
    Ok(root.join("plaesy").join("projects.json"))
}
fn normalized(path: &Path) -> Result<PathBuf, Error> {
    let p = fs::canonicalize(path)?;
    if !p.join(".plaesy").is_dir() {
        return Err(Error::NotProject(p.display().to_string()));
    }
    Ok(p)
}
fn with_registry<T>(f: impl FnOnce(&Path) -> Result<T, Error>) -> Result<T, Error> {
    let file = registry_file()?;
    let parent = file.parent().expect("registry file has parent");
    fs::create_dir_all(parent)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join(".projects.lock"))?;
    lock.lock()?;
    let result = f(&file);
    let _ = lock.unlock();
    result
}
fn read(file: &Path) -> Result<Vec<Project>, Error> {
    if !file.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_slice(&fs::read(file)?)?)
}
fn write(file: &Path, entries: &[Project]) -> Result<(), Error> {
    let parent = file.parent().expect("registry file has parent");
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
/// Resolve a registered project by stable ID or canonical path.
pub fn resolve(selector: &str) -> Result<Project, Error> {
    list()?
        .into_iter()
        .find(|p| p.id == selector || p.path == Path::new(selector))
        .ok_or_else(|| Error::NotFound(selector.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_requires_existing_plaesy_root() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(normalized(dir.path()), Err(Error::NotProject(_))));
        fs::create_dir(dir.path().join(".plaesy")).unwrap();
        assert_eq!(
            normalized(dir.path()).unwrap(),
            fs::canonicalize(dir.path()).unwrap()
        );
    }

    #[test]
    fn registry_records_can_be_filtered_to_existing_plaesy_roots() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("project");
        fs::create_dir_all(project_path.join(".plaesy")).unwrap();
        let canonical_path = fs::canonicalize(&project_path).unwrap();
        let project = Project {
            id: "p-test".into(),
            path: canonical_path.clone(),
        };
        let missing = Project {
            id: "p-missing".into(),
            path: dir.path().join("missing"),
        };
        let registry = dir.path().join("projects.json");
        write(&registry, &[project.clone(), missing]).unwrap();

        let valid: Vec<_> = read(&registry)
            .unwrap()
            .into_iter()
            .filter(|entry| normalized(&entry.path).is_ok())
            .collect();
        assert_eq!(valid, vec![project.clone()]);
        assert_eq!(
            valid.iter().find(|entry| entry.id == "p-test"),
            Some(&project)
        );
    }
}
