//! Unit tests for `contracts_tests` split out of `validate.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

#[cfg_attr(not(unix), allow(unused_imports))]
use super::*;

#[cfg(unix)]
#[test]
fn contracts_check_reader_rejects_replaced_task_directory() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let project = temp.path();
    let plaesy_dir = project.join(".plaesy");
    let tasks_dir = plaesy_dir.join("tasks");
    let task_dir = tasks_dir.join("backlog/item");
    std::fs::create_dir_all(&task_dir).unwrap();
    std::fs::write(
        task_dir.join(tasks::CONTRACT_FILE),
        r#"{"id":"item","checklist":[]}"#,
    )
    .unwrap();
    let external = temp.path().join("external");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(
        external.join(tasks::CONTRACT_FILE),
        r#"{"id":"external","checklist":[]}"#,
    )
    .unwrap();
    let root = crate::memory::fs_safety::DirHandle::open_nofollow(&tasks_dir).unwrap();
    let original = tasks_dir.join("backlog/item-original");
    std::fs::rename(&task_dir, &original).unwrap();
    symlink(&external, &task_dir).unwrap();
    let contract = tasks::contracts::ContractRef {
        spec: String::new(),
        status: tasks::Status::Backlog,
        task: "item".into(),
        path: task_dir.join(tasks::CONTRACT_FILE),
    };

    assert!(load_task_contract(&root, &contract).is_err());
}
