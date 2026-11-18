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
#[path = "contracts_test.rs"]
mod contracts_test;
