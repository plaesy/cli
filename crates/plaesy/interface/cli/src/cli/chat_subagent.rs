//! `/subagent <task>`: a separate model run that works in its own git worktree.
//!
//! The worktree is created from `HEAD` under `.plaesy/subagents/<name>`, detached, so the main
//! checkout is not touched. The subagent gets the same tools as the chat, rooted in that worktree,
//! and its writes need `--allow-writes`. Nothing is merged: the result names the worktree, and the
//! owner decides what to keep.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Folder under the repository where subagent worktrees are created.
#[must_use]
pub fn dir(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("subagents")
}

/// Creates a detached worktree of `HEAD` at `dest`.
///
/// # Errors
///
/// The git error text, or the I/O error when git cannot be started.
pub fn create_worktree(repo: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "--detach"])
        .arg(dest)
        .arg("HEAD")
        .output()
        .map_err(|e| format!("git tidak bisa dijalankan: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// A new name for one subagent run, from the time it starts.
#[must_use]
pub fn new_name() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!("sub-{millis}")
}

#[cfg(test)]
#[path = "chat_subagent_test.rs"]
mod chat_subagent_test;
