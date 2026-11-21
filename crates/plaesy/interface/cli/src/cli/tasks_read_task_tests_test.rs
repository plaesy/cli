//! Unit tests for `read_task_tests` split out of `tasks.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

#[cfg_attr(not(unix), allow(unused_imports))]
use super::*;

#[cfg(unix)]
#[test]
fn default_status_does_not_read_task_file_symlink() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let tasks_dir = root.path().join(".plaesy/tasks");
    let task_dir = tasks_dir.join("backlog/low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    let outside = root.path().join("external-task.md");
    fs::write(&outside, "SYNTHETIC_EXTERNAL_TASK_CONTENT").unwrap();
    symlink(&outside, task_dir.join(tasks::TASK_FILE)).unwrap();

    let result = read_task(&tasks_dir, "low_linked", None);

    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "SYNTHETIC_EXTERNAL_TASK_CONTENT"
    );
}

#[cfg(unix)]
#[test]
fn explicit_status_does_not_read_task_file_symlink() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let tasks = root.path().join(".plaesy/tasks");
    let task_dir = tasks.join("backlog/low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    let outside = root.path().join("external-task.md");
    fs::write(&outside, "SYNTHETIC_EXTERNAL_TASK_CONTENT").unwrap();
    symlink(&outside, task_dir.join(tasks::TASK_FILE)).unwrap();

    let result = read_task(&tasks, "low_linked", Some(Status::Backlog));

    assert!(result.is_err());
}
