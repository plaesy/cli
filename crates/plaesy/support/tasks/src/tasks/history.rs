//! Local version history of a task's `task.md` and `issue.json`.
//!
//! Before a CLI command changes a task, its two files are copied into
//! `.plaesy/history/<task>/<NNNNNN>/`. Each version holds the copies plus a
//! `meta.json` written last, so a version without `meta.json` is an
//! interrupted snapshot and is ignored. The folder is ignored by Git through the
//! existing `.plaesy*/` rule, so the history stays on this machine.
//!
//! Restoring a version first snapshots the current state, so a restore can
//! itself be undone.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::issue::{Issue, IssueGraphLock};
use super::{
    find_task, path_has_symlink_component, tasks_dir, validate_task_name, Status, TasksError,
    TASK_FILE,
};

/// Folder under `.plaesy/` that holds one sub-folder per task.
pub const HISTORY_DIR: &str = "history";
/// Metadata file written last inside each version folder.
pub const META_FILE: &str = "meta.json";
/// Issue metadata file that is versioned next to `task.md`.
pub const ISSUE_FILE: &str = "issue.json";

/// Errors from the task history.
#[derive(Debug, Error)]
pub enum HistoryError {
    #[error(transparent)]
    Tasks(#[from] TasksError),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("version {version} not found for task {task}")]
    VersionNotFound { task: String, version: u32 },
    #[error("corrupt version {version} of task {task}: {reason}")]
    Corrupt {
        task: String,
        version: u32,
        reason: String,
    },
    #[error("refusing symlink in history path {0}")]
    Symlink(String),
    #[error("issue graph lock: {0}")]
    Lock(String),
    #[error("restored issue graph is invalid: {0}")]
    Graph(String),
}

/// One saved version of a task. Stored as `meta.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionMeta {
    /// Version number, starting at 1 and increasing per task.
    pub version: u32,
    /// Status folder the task was in when the version was taken.
    pub status: String,
    /// Command that triggered the snapshot, or the reason for it.
    pub label: String,
    /// Seconds since the Unix epoch.
    pub created_unix: u64,
    /// Files copied into the version folder.
    pub files: Vec<String>,
}

/// Result of [`restore_version`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// Version that was restored.
    pub version: u32,
    /// Version that holds the state from before the restore.
    pub preserved_as: u32,
}

/// Snapshots `task`'s files before a change. Returns `Ok(None)` when the task does
/// not exist, so the command can report that error itself.
///
/// # Errors
///
/// [`HistoryError`] when the task exists but its files cannot be copied.
pub fn snapshot_task(
    project_root: &Path,
    task: &str,
    label: &str,
) -> Result<Option<VersionMeta>, HistoryError> {
    validate_task_name(task)?;
    let tasks = tasks_dir(project_root);
    let Some((status, task_dir)) = find_task(&tasks, task) else {
        return Ok(None);
    };
    snapshot_in(project_root, task, status, &task_dir, label).map(Some)
}

/// Saved versions of `task`, oldest first. Empty when the task has no history.
///
/// # Errors
///
/// [`HistoryError`] on invalid names, symlinked history folders, I/O failures, or
/// a `meta.json` that does not match its folder.
pub fn list_versions(project_root: &Path, task: &str) -> Result<Vec<VersionMeta>, HistoryError> {
    validate_task_name(task)?;
    let root = history_dir(project_root, task);
    refuse_symlinks(&root)?;
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(format!("reading {}", root.display()), error)),
    };
    let mut versions = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_error(format!("reading {}", root.display()), e))?;
        let Some(number) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if let Some(meta) = read_version_meta(&entry.path(), task, number)? {
            versions.push(meta);
        }
    }
    versions.sort_by_key(|meta| meta.version);
    Ok(versions)
}

/// Restores `version` of `task` into the task's current folder.
///
/// The current state is snapshotted first and the restored `task.md` keeps the
/// status of the folder the task is in now. The issue graph is validated after the
/// write; on failure both files go back to their previous contents.
///
/// # Errors
///
/// [`HistoryError::VersionNotFound`] when the version does not exist,
/// [`HistoryError::TaskNotFound`] when the task is gone, [`HistoryError::Graph`]
/// when the restored issue graph is invalid, or I/O failures.
pub fn restore_version(
    project_root: &Path,
    task: &str,
    version: u32,
) -> Result<Restored, HistoryError> {
    validate_task_name(task)?;
    let (saved_task, saved_issue) = load_version(project_root, task, version)?;
    let tasks = tasks_dir(project_root);
    let _graph_lock = IssueGraphLock::acquire(&tasks).map_err(HistoryError::Lock)?;
    let (status, task_dir) =
        find_task(&tasks, task).ok_or_else(|| HistoryError::TaskNotFound(task.to_owned()))?;
    let preserved = snapshot_in(
        project_root,
        task,
        status,
        &task_dir,
        &format!("before restore of v{version}"),
    )?;

    let task_path = task_dir.join(TASK_FILE);
    let issue_path = task_dir.join(ISSUE_FILE);
    let previous_task = read_optional(&task_path)?.unwrap_or_default();
    let previous_issue = read_optional(&issue_path)?;

    let text = String::from_utf8(saved_task).map_err(|error| HistoryError::Corrupt {
        task: task.to_owned(),
        version,
        reason: format!("task.md is not UTF-8: {error}"),
    })?;
    let new_task = with_status(&text, status);
    let applied = apply_restore(
        &tasks,
        &task_path,
        &issue_path,
        new_task.as_bytes(),
        saved_issue.as_deref(),
    );
    if let Err(error) = applied {
        rollback(
            &task_path,
            &issue_path,
            &previous_task,
            previous_issue.as_deref(),
        );
        return Err(error);
    }
    Ok(Restored {
        version,
        preserved_as: preserved.version,
    })
}

fn history_dir(project_root: &Path, task: &str) -> PathBuf {
    project_root.join(".plaesy").join(HISTORY_DIR).join(task)
}

fn snapshot_in(
    project_root: &Path,
    task: &str,
    status: Status,
    task_dir: &Path,
    label: &str,
) -> Result<VersionMeta, HistoryError> {
    let task_bytes = read_optional(&task_dir.join(TASK_FILE))?.ok_or_else(|| HistoryError::Io {
        context: format!("snapshot of {task}"),
        source: io::Error::new(io::ErrorKind::NotFound, "task.md is missing"),
    })?;
    let issue_bytes = read_optional(&task_dir.join(ISSUE_FILE))?;

    let root = history_dir(project_root, task);
    refuse_symlinks(&root)?;
    fs::create_dir_all(&root).map_err(|e| io_error(format!("creating {}", root.display()), e))?;
    refuse_symlinks(&root)?;

    let version = next_version_number(&root)?;
    let dir = root.join(format!("{version:06}"));
    fs::create_dir(&dir).map_err(|e| io_error(format!("creating {}", dir.display()), e))?;
    write_new(&dir.join(TASK_FILE), &task_bytes)?;
    let mut files = vec![TASK_FILE.to_owned()];
    if let Some(bytes) = &issue_bytes {
        write_new(&dir.join(ISSUE_FILE), bytes)?;
        files.push(ISSUE_FILE.to_owned());
    }
    let meta = VersionMeta {
        version,
        status: status.as_str().to_owned(),
        label: label.to_owned(),
        created_unix: now_unix(),
        files,
    };
    let json = serde_json::to_vec_pretty(&meta)
        .map_err(|e| io_error(format!("encoding meta for {task}"), io::Error::other(e)))?;
    // meta.json goes last: its presence marks the version as complete.
    write_new(&dir.join(META_FILE), &json)?;
    Ok(meta)
}

fn load_version(
    project_root: &Path,
    task: &str,
    version: u32,
) -> Result<(Vec<u8>, Option<Vec<u8>>), HistoryError> {
    let root = history_dir(project_root, task);
    refuse_symlinks(&root)?;
    let dir = root.join(format!("{version:06}"));
    if read_version_meta(&dir, task, version)?.is_none() {
        return Err(HistoryError::VersionNotFound {
            task: task.to_owned(),
            version,
        });
    }
    let task_bytes = read_optional(&dir.join(TASK_FILE))?.ok_or_else(|| HistoryError::Corrupt {
        task: task.to_owned(),
        version,
        reason: "task.md is missing".into(),
    })?;
    let issue_bytes = read_optional(&dir.join(ISSUE_FILE))?;
    Ok((task_bytes, issue_bytes))
}

/// Reads `meta.json` of one version folder. `None` when the folder or the file is
/// absent, which covers interrupted snapshots.
fn read_version_meta(
    dir: &Path,
    task: &str,
    version: u32,
) -> Result<Option<VersionMeta>, HistoryError> {
    let Some(bytes) = read_optional(&dir.join(META_FILE))? else {
        return Ok(None);
    };
    let meta: VersionMeta = serde_json::from_slice(&bytes).map_err(|e| HistoryError::Corrupt {
        task: task.to_owned(),
        version,
        reason: e.to_string(),
    })?;
    if meta.version != version {
        return Err(HistoryError::Corrupt {
            task: task.to_owned(),
            version,
            reason: format!("meta.json says version {}", meta.version),
        });
    }
    Ok(Some(meta))
}

fn apply_restore(
    tasks: &Path,
    task_path: &Path,
    issue_path: &Path,
    task_bytes: &[u8],
    issue_bytes: Option<&[u8]>,
) -> Result<(), HistoryError> {
    write_replace(task_path, task_bytes)?;
    match issue_bytes {
        Some(bytes) => write_replace(issue_path, bytes)?,
        None => remove_if_exists(issue_path)?,
    }
    let problems = Issue::validate_collection(tasks).map_err(HistoryError::Graph)?;
    if problems.is_empty() {
        Ok(())
    } else {
        Err(HistoryError::Graph(problems.join("; ")))
    }
}

/// Best-effort return to the state before a failed restore.
fn rollback(task_path: &Path, issue_path: &Path, task: &[u8], issue: Option<&[u8]>) {
    let _ = write_replace(task_path, task);
    match issue {
        Some(bytes) => {
            let _ = write_replace(issue_path, bytes);
        }
        None => {
            let _ = remove_if_exists(issue_path);
        }
    }
}

/// Rewrites the top-level `status:` line of the frontmatter to `status`. Text
/// without frontmatter is returned unchanged.
fn with_status(text: &str, status: Status) -> String {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if lines.first().map(|line| line.trim()) != Some("---") {
        return text.to_owned();
    }
    let Some(end) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, line)| line.trim() == "---")
        .map(|(index, _)| index)
    else {
        return text.to_owned();
    };
    let status_line = format!("status: {}", status.as_str());
    match lines[1..end]
        .iter()
        .position(|line| line.starts_with("status:"))
    {
        Some(offset) => lines[1 + offset] = status_line,
        None => lines.insert(1, status_line),
    }
    let mut out = lines.join(eol);
    out.push_str(eol);
    out
}

/// Next free version number: one past the highest folder, complete or not.
fn next_version_number(root: &Path) -> Result<u32, HistoryError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(1),
        Err(error) => return Err(io_error(format!("reading {}", root.display()), error)),
    };
    let mut highest = 0_u32;
    for entry in entries {
        let entry = entry.map_err(|e| io_error(format!("reading {}", root.display()), e))?;
        if let Some(number) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        {
            highest = highest.max(number);
        }
    }
    highest.checked_add(1).ok_or_else(|| {
        io_error(
            format!("version limit reached in {}", root.display()),
            io::Error::other("version number overflow"),
        )
    })
}

/// Reads a regular file. `None` when it does not exist. Symlinks are refused.
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, HistoryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(HistoryError::Symlink(path.display().to_string()))
        }
        Ok(metadata) if !metadata.is_file() => Err(io_error(
            format!("reading {}", path.display()),
            io::Error::new(io::ErrorKind::InvalidData, "not a regular file"),
        )),
        Ok(_) => fs::read(path)
            .map(Some)
            .map_err(|e| io_error(format!("reading {}", path.display()), e)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(format!("reading {}", path.display()), error)),
    }
}

fn refuse_symlinks(path: &Path) -> Result<(), HistoryError> {
    if path_has_symlink_component(path) {
        Err(HistoryError::Symlink(path.display().to_string()))
    } else {
        Ok(())
    }
}

/// Creates a new file and fails if it already exists.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), HistoryError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| io_error(format!("creating {}", path.display()), e))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| io_error(format!("writing {}", path.display()), e))
}

/// Replaces a file through a temporary sibling, so a reader never sees a half-written file.
fn write_replace(path: &Path, bytes: &[u8]) -> Result<(), HistoryError> {
    let Some(file_name) = path.file_name() else {
        return Err(io_error(
            format!("no file name in {}", path.display()),
            io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"),
        ));
    };
    refuse_symlinks(path.parent().unwrap_or(path))?;
    let temp = path.with_file_name(format!(
        ".{}.history-{}.tmp",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    let _ = fs::remove_file(&temp);
    write_new(&temp, bytes)?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(io_error(format!("replacing {}", path.display()), error));
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), HistoryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(HistoryError::Symlink(path.display().to_string()))
        }
        Ok(_) => {
            fs::remove_file(path).map_err(|e| io_error(format!("removing {}", path.display()), e))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(format!("reading {}", path.display()), error)),
    }
}

fn io_error(context: String, source: io::Error) -> HistoryError {
    HistoryError::Io { context, source }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
#[path = "history_test.rs"]
mod history_test;
