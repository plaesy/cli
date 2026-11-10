//! Environment and path preconditions.

use std::env;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::log::Logger;

/// A precondition that does not hold.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    /// An executable is not on `PATH`.
    #[error("required {description} not found: {name}")]
    CommandMissing { description: String, name: String },
    /// A required file is absent or is a directory.
    #[error("required {description} not found: {}", path.display())]
    FileMissing { description: String, path: PathBuf },
    /// A required directory is absent or is not a directory.
    #[error("required {description} not found: {}", path.display())]
    DirectoryMissing { description: String, path: PathBuf },
    /// Running as root on a POSIX system.
    #[error("running as root is not permitted")]
    RunningAsRoot,
}

/// Finds `name` on `PATH`, honouring `PATHEXT` on Windows.
#[must_use]
pub fn find_command(name: &str) -> Option<PathBuf> {
    let dirs = env::var_os("PATH")?;
    let exts = executable_suffixes();
    env::split_paths(&dirs).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(windows)]
fn executable_suffixes() -> Vec<String> {
    let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
    std::iter::once(String::new())
        .chain(
            pathext
                .split(';')
                .filter(|e| !e.is_empty())
                .map(str::to_owned),
        )
        .collect()
}

#[cfg(not(windows))]
fn executable_suffixes() -> Vec<String> {
    vec![String::new()]
}

/// Succeeds when `name` is an executable on `PATH`.
///
/// # Errors
///
/// Returns [`ValidationError::CommandMissing`] when it is not found.
pub fn command_exists(
    name: &str,
    description: &str,
    log: &Logger,
) -> Result<PathBuf, ValidationError> {
    match find_command(name) {
        Some(path) => {
            log.debug(&format!("Found {name}: {}", path.display()));
            Ok(path)
        }
        None => {
            let err = ValidationError::CommandMissing {
                description: non_empty(description, "command"),
                name: name.to_owned(),
            };
            log.error(&err.to_string());
            Err(err)
        }
    }
}

/// Succeeds when `path` is an existing non-directory.
///
/// # Errors
///
/// Returns [`ValidationError::FileMissing`] otherwise.
pub fn file_exists(path: &Path, description: &str, log: &Logger) -> Result<(), ValidationError> {
    if path.is_file() {
        log.debug(&format!("Found {description}: {}", path.display()));
        return Ok(());
    }
    let err = ValidationError::FileMissing {
        description: non_empty(description, "file"),
        path: path.to_path_buf(),
    };
    log.error(&err.to_string());
    Err(err)
}

/// Succeeds when `path` is an existing directory.
///
/// # Errors
///
/// Returns [`ValidationError::DirectoryMissing`] otherwise.
pub fn directory_exists(
    path: &Path,
    description: &str,
    log: &Logger,
) -> Result<(), ValidationError> {
    if path.is_dir() {
        log.debug(&format!("Found {description}: {}", path.display()));
        return Ok(());
    }
    let err = ValidationError::DirectoryMissing {
        description: non_empty(description, "directory"),
        path: path.to_path_buf(),
    };
    log.error(&err.to_string());
    Err(err)
}

/// Refuses to run as root. Always succeeds on Windows, which has no equivalent.
///
/// # Errors
///
/// Returns [`ValidationError::RunningAsRoot`] when the effective uid is 0.
pub fn not_root(log: &Logger) -> Result<(), ValidationError> {
    #[cfg(unix)]
    {
        // SAFETY: geteuid has no preconditions and cannot fail.
        if unsafe { libc::geteuid() } == 0 {
            let err = ValidationError::RunningAsRoot;
            log.error("This script should not be run as root for security reasons.");
            return Err(err);
        }
    }
    let _ = log;
    Ok(())
}

fn non_empty(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> Logger {
        Logger::new("test")
    }

    #[test]
    fn file_check_rejects_directories_and_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(file_exists(&file, "", &quiet()).is_ok());
        assert!(file_exists(dir.path(), "", &quiet()).is_err());
        let err = file_exists(&dir.path().join("nope"), "", &quiet()).unwrap_err();
        assert!(
            matches!(err, ValidationError::FileMissing { ref description, .. } if description == "file")
        );
    }

    #[test]
    fn directory_check_rejects_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(directory_exists(dir.path(), "", &quiet()).is_ok());
        assert!(directory_exists(&file, "", &quiet()).is_err());
    }

    #[test]
    fn missing_command_names_the_command() {
        let err = command_exists("plaesy-definitely-not-a-command-xyz", "", &quiet()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "required command not found: plaesy-definitely-not-a-command-xyz"
        );
    }
}
