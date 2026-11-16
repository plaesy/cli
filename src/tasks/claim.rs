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
mod tests {
    use super::*;
    use crate::tasks::{add_task, find_task, tasks_dir};

    fn assert_workers_claim_one_task_only_once() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        let tasks = tasks_dir(dir.path());
        add_task(&tasks, "auth", "low", "only", "Only task").unwrap();
        let tasks = std::sync::Arc::new(tasks);

        let workers: Vec<_> = (0..8)
            .map(|_| {
                let tasks = std::sync::Arc::clone(&tasks);
                std::thread::spawn(move || claim_task(&tasks, "auth").unwrap())
            })
            .collect();
        let claimed: Vec<_> = workers
            .into_iter()
            .filter_map(|worker| worker.join().unwrap())
            .collect();

        assert_eq!(claimed, ["low_only"]);
        assert_eq!(find_task(&tasks, "low_only").unwrap().0, Status::Doing);
    }

    #[test]
    fn concurrent_workers_claim_a_task_only_once() {
        assert_workers_claim_one_task_only_once();
    }

    #[test]
    fn concurrent_workers_claim_a_task_once_across_repeated_runs() {
        for _ in 0..25 {
            assert_workers_claim_one_task_only_once();
        }
    }

    #[test]
    fn claim_promotes_then_moves_one_task_to_doing() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        let tasks = tasks_dir(dir.path());
        add_task(&tasks, "auth", "high", "a", "A").unwrap();
        add_task(&tasks, "auth", "low", "b", "B").unwrap();

        assert_eq!(
            claim_task(&tasks, "auth").unwrap().as_deref(),
            Some("high_a")
        );
        assert_eq!(find_task(&tasks, "high_a").unwrap().0, Status::Doing);
        assert_eq!(
            claim_task(&tasks, "auth").unwrap().as_deref(),
            Some("low_b")
        );
        assert_eq!(claim_task(&tasks, "auth").unwrap(), None);
    }

    #[test]
    fn a_live_lock_blocks_the_claim_and_is_released_when_handle_closes() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        let tasks = tasks_dir(dir.path());
        add_task(&tasks, "auth", "low", "c", "C").unwrap();
        fs::create_dir_all(tasks.join("todo")).unwrap();
        fs::rename(tasks.join("backlog/low_c"), tasks.join("todo/low_c")).unwrap();

        fs::create_dir_all(tasks.join(".claims")).unwrap();
        let lock_path = tasks.join(".claims/low_c.lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .unwrap();
        lock.lock().unwrap();
        assert!(matches!(acquire(&tasks, "low_c").unwrap(), Acquire::Held));
        assert_eq!(claim_task(&tasks, "auth").unwrap(), None);
        drop(lock);
        assert_eq!(
            claim_task(&tasks, "auth").unwrap().as_deref(),
            Some("low_c")
        );
        assert!(lock_path.exists(), "lock inode path must remain stable");
    }

    #[cfg(unix)]
    #[test]
    fn claim_rejects_a_symlink_claims_directory() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        let tasks = tasks_dir(dir.path());
        add_task(&tasks, "auth", "low", "claims-dir", "Claims dir").unwrap();
        let outside = dir.path().join("outside-claims");
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, tasks.join(".claims")).unwrap();

        let result = claim_task(&tasks, "auth");
        assert!(
            result.is_err(),
            "claim must reject a symlink claims directory"
        );
        assert!(!outside.join("low_claims-dir.lock").exists());
        assert_eq!(
            find_task(&tasks, "low_claims-dir").unwrap().0,
            Status::Backlog
        );
    }

    #[cfg(unix)]
    #[test]
    fn claim_does_not_follow_a_lock_symlink_outside_claim_directory() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        let tasks = tasks_dir(dir.path());
        add_task(&tasks, "auth", "low", "symlink", "Symlink").unwrap();
        fs::create_dir_all(tasks.join(".claims")).unwrap();
        let outside = dir.path().join("outside.lock");
        File::create(&outside).unwrap();
        symlink(&outside, tasks.join(".claims/low_symlink.lock")).unwrap();

        let result = claim_task(&tasks, "auth");
        assert!(result.is_err(), "claim must reject symlink lock paths");
        assert_eq!(find_task(&tasks, "low_symlink").unwrap().0, Status::Backlog);
    }
}
