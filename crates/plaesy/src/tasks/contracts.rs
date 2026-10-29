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
            if !path.is_file() {
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
