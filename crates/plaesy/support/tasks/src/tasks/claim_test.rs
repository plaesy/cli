//! Unit tests for `tests` split out of `claim.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::{add_task, find_task, tasks_dir};

fn assert_workers_claim_one_task_only_once() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
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
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
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
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
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

    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
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

    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
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
