//! Unit tests for `tests` split out of `contracts.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::{add_task, tasks_dir};

#[cfg(unix)]
#[test]
fn symlinked_task_directory_does_not_expose_external_contract() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    let tasks = tasks_dir(dir.path());
    let status_dir = tasks.join("backlog");
    std::fs::create_dir_all(&status_dir).unwrap();
    let external = dir.path().join("external-task");
    std::fs::create_dir_all(&external).unwrap();
    std::fs::write(
        external.join(super::super::TASK_FILE),
        "---\nspec: outside\n---\n",
    )
    .unwrap();
    std::fs::write(external.join(CONTRACT_FILE), "{\"checklist\":[]}").unwrap();
    symlink(&external, status_dir.join("low_external")).unwrap();

    assert!(list_contracts(&tasks, "").unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn symlinked_contract_is_not_listed() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    let tasks = tasks_dir(dir.path());
    let name = add_task(&tasks, "auth", "low", "symlink", "Symlink").unwrap();
    let contract = tasks.join("backlog").join(name).join(CONTRACT_FILE);
    std::fs::remove_file(&contract).unwrap();
    let external = dir.path().join("external-contract.json");
    std::fs::write(&external, "{\"checklist\":[]}").unwrap();
    symlink(&external, &contract).unwrap();

    assert!(list_contracts(&tasks, "").unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn symlinked_task_metadata_does_not_supply_contract_spec() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    let tasks = tasks_dir(dir.path());
    let name = add_task(&tasks, "auth", "low", "metadata", "Metadata").unwrap();
    let task_folder = tasks.join("backlog").join(name);
    let task_file = task_folder.join(super::super::TASK_FILE);
    std::fs::remove_file(&task_file).unwrap();
    let external = dir.path().join("external-task.md");
    std::fs::write(&external, "---\nspec: outside\n---\n").unwrap();
    symlink(&external, &task_file).unwrap();

    assert!(list_contracts(&tasks, "outside").unwrap().is_empty());
}

#[test]
fn ignores_non_regular_contract_and_task_entries() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    let tasks = tasks_dir(dir.path());
    let status_dir = tasks.join("backlog");
    let task_dir = status_dir.join("low_invalid");
    std::fs::create_dir_all(&task_dir).unwrap();
    std::fs::write(task_dir.join(CONTRACT_FILE), "{}").unwrap();
    assert!(list_contracts(&tasks, "").unwrap().is_empty());

    std::fs::write(
        task_dir.join(super::super::TASK_FILE),
        "---\nspec: valid\n---\n",
    )
    .unwrap();
    std::fs::remove_file(task_dir.join(CONTRACT_FILE)).unwrap();
    std::fs::create_dir(task_dir.join(CONTRACT_FILE)).unwrap();
    assert!(list_contracts(&tasks, "").unwrap().is_empty());
}

#[test]
fn lists_only_tasks_with_a_contract_and_filters_by_spec() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    let tasks = tasks_dir(dir.path());
    add_task(&tasks, "auth", "low", "a", "A").unwrap();
    add_task(&tasks, "billing", "low", "b", "B").unwrap();
    std::fs::remove_file(tasks.join("backlog/low_b/contracts.json")).unwrap();

    let all = list_contracts(&tasks, "").unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].task, "low_a");
    assert_eq!(list_contracts(&tasks, "billing").unwrap().len(), 0);
    assert_eq!(list_contracts(&tasks, "auth").unwrap().len(), 1);
}
