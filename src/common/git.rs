//! Git repository queries.
//!
//! Port of `scripts/internal/common/gitpaths.go`. Under Git Bash on Windows,
//! git reports paths as MSYS2 mounts (`/c/Users/...`), which must be mapped to
//! native paths before the filesystem can be used.

use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

/// Failure to run a git query.
#[derive(Debug, Error)]
pub enum GitError {
    /// git could not be started or exited non-zero.
    #[error("git {args}: {message}")]
    Failed { args: String, message: String },
}

/// Converts a git-reported path into a native path.
///
/// On Windows, `/c/Users/x` becomes `C:\Users\x`. Other platforms are unchanged.
#[must_use]
pub fn to_native_path(raw: &str) -> PathBuf {
    if cfg!(windows) {
        if let Some(native) = msys_to_windows(raw) {
            return PathBuf::from(native);
        }
        return PathBuf::from(raw.replace('/', "\\"));
    }
    PathBuf::from(raw)
}

fn msys_to_windows(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'/' || bytes[2] != b'/' || !bytes[1].is_ascii_alphabetic() {
        return None;
    }
    let drive = (bytes[1] as char).to_ascii_uppercase();
    Some(format!("{drive}:\\{}", raw[3..].replace('/', "\\")))
}

fn git_output(args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| GitError::Failed {
            args: args.join(" "),
            message: e.to_string(),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            output.status.to_string()
        } else {
            stderr
        };
        return Err(GitError::Failed {
            args: args.join(" "),
            message,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Returns the top-level directory of the current repository.
///
/// # Errors
///
/// Returns [`GitError`] when the working directory is not inside a repository.
pub fn repo_root() -> Result<PathBuf, GitError> {
    let raw = git_output(&["rev-parse", "--show-toplevel"])?;
    let root = to_native_path(&raw);
    if root.exists() {
        return Ok(root);
    }
    // Git Bash can print a POSIX mount that Windows cannot resolve. The same root
    // is available relative to the working directory, which is a native path.
    if let Some(repaired) = root_from_cwd() {
        return Ok(repaired);
    }
    Ok(root)
}

fn root_from_cwd() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let cdup = git_output(&["rev-parse", "--show-cdup"]).ok()?;
    let candidate = if cdup.is_empty() { cwd } else { cwd.join(cdup) };
    candidate.exists().then_some(candidate)
}

/// Returns the current branch name.
///
/// Falls back to `git branch --show-current` on an unborn branch, where
/// `rev-parse` has no revision to resolve.
///
/// # Errors
///
/// Returns [`GitError`] when neither form succeeds.
pub fn current_branch() -> Result<String, GitError> {
    match git_output(&["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(branch) => Ok(branch),
        Err(_) => git_output(&["branch", "--show-current"]),
    }
}

/// Returns whether `path` is inside a git work tree. Used for non-fatal warnings.
#[must_use]
pub fn is_repo(path: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn msys_drive_prefix_maps_to_a_drive_letter() {
        assert_eq!(
            msys_to_windows("/c/Users/me/repo").as_deref(),
            Some("C:\\Users\\me\\repo")
        );
        assert_eq!(msys_to_windows("/D/x").as_deref(), Some("D:\\x"));
    }

    #[test]
    fn non_drive_paths_are_not_rewritten() {
        assert_eq!(msys_to_windows("/tmp/repo"), None);
        assert_eq!(msys_to_windows("C:\\already\\native"), None);
        assert_eq!(msys_to_windows("/c"), None);
    }
}
