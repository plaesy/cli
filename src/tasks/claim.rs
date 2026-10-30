//! Exclusive claim of a task by one worker.
//!
//! Exclusivity comes from a lock directory per task, created with
//! `create_dir`, which fails for every caller but one. A rename alone is not
//! enough: on Windows, concurrent renames of one folder can all report success.
//! The lock is held only while the task moves from `todo` to `doing`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{for_spec, list_tasks, move_task, priority_rank, Status, TasksError};

/// Age after which a lock is treated as left behind by a dead worker.
const CLAIM_STALE: Duration = Duration::from_secs(30 * 60);

/// Removes its lock directory when dropped.
struct ClaimLock {
    path: PathBuf,
}

impl Drop for ClaimLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

/// Outcome of trying to take one lock.
enum Acquire {
    Taken(ClaimLock),
    /// Another live worker holds it.
    Held,
}

fn acquire(tasks_dir: &Path, name: &str) -> Result<Acquire, TasksError> {
    let locks = tasks_dir.join(".claims");
    fs::create_dir_all(&locks)
        .map_err(|e| TasksError::io(format!("creating {}", locks.display()), e))?;
    let path = locks.join(format!("{name}.lock"));
    for _ in 0..2 {
        match fs::create_dir(&path) {
            Ok(()) => return Ok(Acquire::Taken(ClaimLock { path })),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(TasksError::io(format!("locking {name}"), e)),
        }
        let modified = match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(time) => time,
            // Released between our create and our stat: try again.
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(TasksError::io(format!("inspecting {name} lock"), e)),
        };
        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default();
        if age < CLAIM_STALE {
            return Ok(Acquire::Held);
        }
        let _ = fs::remove_dir(&path);
    }
    Ok(Acquire::Held)
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
/// returns its name. When `todo` has nothing claimable, promotes the next backlog
/// task once and tries again. `None` when nothing is left.
///
/// # Errors
///
/// I/O failures or invalid task state while moving folders.
pub fn claim_task(tasks_dir: &Path, spec: &str) -> Result<Option<String>, TasksError> {
    for _ in 0..2 {
        let mut names = list_tasks(tasks_dir, Status::Todo)?.unwrap_or_default();
        names = for_spec(tasks_dir, Status::Todo, names, spec);
        names.sort_by_key(|name| priority_rank(name));
        for name in names {
            if claim_one(tasks_dir, &name)? {
                return Ok(Some(name));
            }
        }
        if !tasks_dir.join("backlog").exists() {
            return Ok(None);
        }
        if super::pop_next_task(tasks_dir, spec)?.is_none() {
            return Ok(None);
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{add_task, find_task, tasks_dir};

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
    fn a_live_lock_blocks_the_claim_and_a_stale_one_is_taken_over() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        let tasks = tasks_dir(dir.path());
        add_task(&tasks, "auth", "low", "c", "C").unwrap();
        fs::create_dir_all(tasks.join("todo")).unwrap();
        fs::rename(tasks.join("backlog/low_c"), tasks.join("todo/low_c")).unwrap();

        let lock = tasks.join(".claims/low_c.lock");
        fs::create_dir_all(&lock).unwrap();
        assert!(matches!(acquire(&tasks, "low_c").unwrap(), Acquire::Held));
        assert_eq!(claim_task(&tasks, "auth").unwrap(), None);
        fs::remove_dir(&lock).unwrap();
        assert_eq!(
            claim_task(&tasks, "auth").unwrap().as_deref(),
            Some("low_c")
        );
    }
}
