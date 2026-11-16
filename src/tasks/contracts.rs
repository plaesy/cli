//! Task folders that carry a `contracts.json` checklist.

use std::path::{Path, PathBuf};

use super::{list_tasks, spec_of, Status, TasksError, CONTRACT_FILE, STATUSES};

/// One task folder that holds a `contracts.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractRef {
    pub spec: String,
    pub status: Status,
    pub task: String,
    pub path: PathBuf,
}

/// Every task with a `contracts.json`, ordered by status then name. A non-empty
/// `spec` keeps only that spec's tasks.
///
/// # Errors
///
/// I/O failures while listing status folders.
pub fn list_contracts(tasks_dir: &Path, spec: &str) -> Result<Vec<ContractRef>, TasksError> {
    let mut refs = Vec::new();
    for status in STATUSES {
        for name in list_tasks(tasks_dir, status)?.unwrap_or_default() {
            let path = tasks_dir
                .join(status.as_str())
                .join(&name)
                .join(CONTRACT_FILE);
            if !std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
                continue;
            }
            let task_file = tasks_dir
                .join(status.as_str())
                .join(&name)
                .join(super::TASK_FILE);
            if !std::fs::symlink_metadata(&task_file).is_ok_and(|metadata| metadata.is_file()) {
                continue;
            }
            let task_spec = spec_of(tasks_dir, status, &name);
            if !spec.is_empty() && task_spec != spec {
                continue;
            }
            refs.push(ContractRef {
                spec: task_spec,
                status,
                task: name,
                path,
            });
        }
    }
    Ok(refs)
}

#[cfg(test)]
mod tests {
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
}
