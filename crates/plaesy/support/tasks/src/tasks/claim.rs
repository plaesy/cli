//! Exclusive claim of a task by one worker.
//!
//! Exclusivity comes from an advisory OS file lock per task. The lock is held
//! while the task moves to `doing`; other writers must use the same protocol.

use std::fs::{self, File, OpenOptions};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use super::{for_spec, list_tasks, move_task, priority_rank, Status, TasksError};

/// Holds an advisory lock until this guard is dropped.
struct ClaimLock {
    _file: File,
}

/// Outcome of trying to take one lock.
enum Acquire {
    Taken(ClaimLock),
    /// Another live worker holds it.
    Held,
}

fn acquire(tasks_dir: &Path, name: &str) -> Result<Acquire, TasksError> {
    let locks = tasks_dir.join(".claims");
    match fs::symlink_metadata(&locks) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(TasksError::io(
                format!(
                    "claim lock directory {} must not be a symlink",
                    locks.display()
                ),
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "symlink not allowed"),
            ));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(TasksError::io(
                format!("claim lock path {} is not a directory", locks.display()),
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a directory"),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::create_dir(&locks) {
                Ok(()) => {}
                Err(create_error) if create_error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&locks).map_err(|e| {
                        TasksError::io(format!("inspecting {}", locks.display()), e)
                    })?;
                    if metadata.file_type().is_symlink() || !metadata.is_dir() {
                        return Err(TasksError::io(
                            format!(
                                "claim lock path {} is not a real directory",
                                locks.display()
                            ),
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "not a real directory",
                            ),
                        ));
                    }
                }
                Err(create_error) => {
                    return Err(TasksError::io(
                        format!("creating {}", locks.display()),
                        create_error,
                    ));
                }
            }
        }
        Err(error) => {
            return Err(TasksError::io(
                format!("inspecting {}", locks.display()),
                error,
            ));
        }
    }
    let path = locks.join(format!("{name}.lock"));
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options
        .open(&path)
        .map_err(|e| TasksError::io(format!("opening {name} lock"), e))?;
    match file.try_lock() {
        Ok(()) => Ok(Acquire::Taken(ClaimLock { _file: file })),
        Err(std::fs::TryLockError::WouldBlock) => Ok(Acquire::Held),
        Err(std::fs::TryLockError::Error(e)) => Err(TasksError::io(format!("locking {name}"), e)),
    }
}

/// Takes the lock for `name`, then moves the task from `todo` to `doing`.
/// Returns `Ok(false)` when another worker holds the lock or already moved it.
fn claim_one(tasks_dir: &Path, name: &str) -> Result<bool, TasksError> {
    let lock = match acquire(tasks_dir, name)? {
        Acquire::Taken(lock) => lock,
        Acquire::Held => return Ok(false),
    };
    match move_task(tasks_dir, name, Status::Todo, Status::Doing) {
        Ok(()) => {
            drop(lock);
            Ok(true)
        }
        Err(TasksError::NotFoundIn { .. } | TasksError::WrongStatus { .. }) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Claims the highest-priority `todo` task of `spec` (any spec when empty) and
/// returns its name. When `todo` has nothing claimable, claims a backlog task
/// and moves it directly to `doing`. Returns `None` when none are claimable.
///
/// # Errors
///
/// I/O failures or invalid task state while moving folders.
pub fn claim_task(tasks_dir: &Path, spec: &str) -> Result<Option<String>, TasksError> {
    let mut names = list_tasks(tasks_dir, Status::Todo)?.unwrap_or_default();
    names = for_spec(tasks_dir, Status::Todo, names, spec);
    names.sort_by_key(|name| priority_rank(name));
    for name in names {
        if claim_one(tasks_dir, &name)? {
            return Ok(Some(name));
        }
    }
    let mut backlog = list_tasks(tasks_dir, Status::Backlog)?.unwrap_or_default();
    backlog = for_spec(tasks_dir, Status::Backlog, backlog, spec);
    backlog.sort_by_key(|name| priority_rank(name));
    for name in backlog {
        if claim_backlog_one(tasks_dir, &name)? {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

fn claim_backlog_one(tasks_dir: &Path, name: &str) -> Result<bool, TasksError> {
    let _lock = match acquire(tasks_dir, name)? {
        Acquire::Taken(lock) => lock,
        Acquire::Held => return Ok(false),
    };
    match move_task(tasks_dir, name, Status::Backlog, Status::Todo) {
        Ok(()) => match move_task(tasks_dir, name, Status::Todo, Status::Doing) {
            Ok(()) => Ok(true),
            Err(error) => Err(error),
        },
        Err(TasksError::NotFoundIn { .. } | TasksError::WrongStatus { .. }) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
#[path = "claim_test.rs"]
mod claim_test;
