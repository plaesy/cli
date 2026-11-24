//! File storage for local users at `.plaesy/users.json` (spec R5, single organization).
//!
//! The file holds salts and digests only. A missing file loads as an empty store.
//! Saves write a temporary file and rename it over the old one, so a crash never
//! leaves a half-written users file. On Unix the file is created with mode 0600.

use plaesy_domain_identity::users::UserStore;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Location of the users file, relative to the project root.
pub const USERS_RELATIVE_PATH: &str = ".plaesy/users.json";

/// Errors when reading or writing the users file.
#[derive(Debug, Error)]
pub enum UserFileError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("users file is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Returns the users file path under `project_root`.
#[must_use]
pub fn users_path(project_root: &Path) -> PathBuf {
    project_root.join(USERS_RELATIVE_PATH)
}

/// Loads the store at `path`. A missing file yields an empty store.
///
/// # Errors
///
/// Returns [`UserFileError::Io`] for read failures and [`UserFileError::Json`] for malformed content.
pub fn load_users(path: &Path) -> Result<UserStore, UserFileError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(UserStore::from_json(&text)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(UserStore::new()),
        Err(error) => Err(error.into()),
    }
}

/// Writes the store to `path`, creating the parent directory when needed.
///
/// # Errors
///
/// Returns [`UserFileError::Io`] when the file cannot be written or renamed, and
/// [`UserFileError::Json`] when serialization fails.
pub fn save_users(path: &Path, store: &UserStore) -> Result<(), UserFileError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = store.to_json()?;
    let temporary = path.with_extension("json.tmp");
    write_private(&temporary, text.as_bytes())?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    std::fs::write(path, bytes)
}

#[cfg(test)]
#[path = "users_test.rs"]
mod users_test;
