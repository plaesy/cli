//! Task lifecycle for a spec.
//!
//! A task is a folder:
//!
//! ```text
//! .plaesy/tasks/<status>/<priority>_<category>-<title>/
//!   task.md          status, grouping, and the topic the task belongs to
//!   contracts.json   the checklist that says when it is done
//!
//! .plaesy/spec/<topic>/spec.md   what the topic is for (requirements R1..Rn)
//! .plaesy/plan/<topic>/plan.md   how the topic is met and verified
//! ```
//!
//! The status is the folder the task sits in. Moving a task moves its folder.
//! Spec and plan live under `.plaesy/spec/` and `.plaesy/plan/`, one folder per
//! topic, so several tasks can share one topic.
//! Port of `scripts/internal/tasks` (Go).

pub mod check;
pub mod claim;
pub mod comments;
pub mod contracts;
pub mod inflight;
pub mod issue;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use thiserror::Error;

mod creation;
mod frontmatter;
mod movement;
pub use creation::*;
pub use frontmatter::*;
pub use movement::*;

/// File every task folder holds: status, grouping, and its topic.
pub const TASK_FILE: &str = "task.md";
/// Checklist file every task folder may hold.
pub const CONTRACT_FILE: &str = "contracts.json";
/// Folder under the project's `.plaesy/` that holds one spec per topic.
pub const SPEC_DIR: &str = "spec";
/// Folder under the project's `.plaesy/` that holds one plan per topic.
pub const PLAN_DIR: &str = "plan";
/// Spec document inside a topic folder.
pub const SPEC_FILE: &str = "spec.md";
/// Plan document inside a topic folder.
pub const PLAN_FILE: &str = "plan.md";

/// Errors from task operations.
#[derive(Debug, Error)]
pub enum TasksError {
    #[error("invalid status: {0} (must be: backlog, todo, doing, done, blocked)")]
    InvalidStatus(String),
    #[error("invalid priority: {0} (must be: critical, high, medium, low)")]
    InvalidPriority(String),
    #[error("invalid task slug {0:?}")]
    InvalidSlug(String),
    #[error("task not found: {0}")]
    NotFound(String),
    #[error("task not found in {status}: {task}")]
    NotFoundIn { status: Status, task: String },
    #[error("{task} is in {actual}; {verb} needs {needed}")]
    WrongStatus {
        task: String,
        actual: Status,
        verb: String,
        needed: Status,
    },
    #[error("task already exists in {status}: {task}")]
    AlreadyExists { status: Status, task: String },
    #[error("task already exists: {0}")]
    Duplicate(String),
    #[error("comment text is empty")]
    EmptyComment,
    #[error("comment {id} not found on {task}")]
    CommentNotFound { task: String, id: u64 },
    #[error("{context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("task {0} has no topic; every task needs one (topic: in task.md)")]
    NoTopic(String),
    #[error("{0} missing reference(s)")]
    MissingReferences(usize),
    #[error("project root not found (.plaesy directory not found)")]
    ProjectRootNotFound,
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
}

impl TasksError {
    fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

/// Status folder of a task, in lifecycle order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    Backlog,
    Todo,
    Doing,
    Review,
    Done,
    Blocked,
}

/// All statuses, in lifecycle order.
pub const STATUSES: [Status; 6] = [
    Status::Backlog,
    Status::Todo,
    Status::Doing,
    Status::Review,
    Status::Done,
    Status::Blocked,
];

impl Status {
    /// Folder name for this status.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Review => "review",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Status {
    type Err = TasksError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        STATUSES
            .into_iter()
            .find(|status| status.as_str() == s)
            .ok_or_else(|| TasksError::InvalidStatus(s.to_owned()))
    }
}

/// Priority prefix of a task name, most urgent first.
pub const PRIORITIES: [&str; 4] = ["critical", "high", "medium", "low"];

/// Validates a priority prefix.
///
/// # Errors
///
/// [`TasksError::InvalidPriority`] when it is not one of [`PRIORITIES`].
pub fn validate_priority(priority: &str) -> Result<(), TasksError> {
    if PRIORITIES.contains(&priority) {
        Ok(())
    } else {
        Err(TasksError::InvalidPriority(priority.to_owned()))
    }
}

/// Sort rank of a task name by its priority prefix. Unknown prefixes rank last.
#[must_use]
pub fn priority_rank(name: &str) -> usize {
    PRIORITIES
        .iter()
        .position(|p| name.starts_with(&format!("{p}_")))
        .unwrap_or(PRIORITIES.len())
}

/// One task with the fields a board shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRef {
    pub spec: String,
    pub status: Status,
    pub name: String,
    pub title: String,
    pub phase: String,
}

/// One inconsistency found by [`check_integrity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub task: String,
    pub message: String,
}

/// Project root for a `.plaesy/tasks` folder: `<root>/.plaesy/tasks` → `<root>`.
#[must_use]
pub fn project_root_of(tasks_dir: &Path) -> PathBuf {
    tasks_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or(tasks_dir)
        .to_path_buf()
}

/// Walks up from `start` to the nearest directory that holds `.plaesy`.
///
/// # Errors
///
/// [`TasksError::ProjectRootNotFound`] when no ancestor has one before the Git repository boundary.
pub fn find_project_root(start: &Path) -> Result<PathBuf, TasksError> {
    for dir in start.ancestors() {
        if dir.join(".plaesy").is_dir() {
            return Ok(dir.to_path_buf());
        }
        if dir.join(".git").exists() {
            break;
        }
    }
    Err(TasksError::ProjectRootNotFound)
}

/// `<root>/.plaesy/tasks`, where every task of every spec lives.
#[must_use]
pub fn tasks_dir(project_root: &Path) -> PathBuf {
    project_root.join(".plaesy").join("tasks")
}

/// Task folder names in one status, sorted. `None` when the status folder is absent.
///
/// # Errors
///
/// I/O failures other than a missing folder.
pub fn list_tasks(tasks_dir: &Path, status: Status) -> Result<Option<Vec<String>>, TasksError> {
    if path_has_symlink_component(tasks_dir) {
        return Err(TasksError::io(
            format!("refusing symlinked tasks root {}", tasks_dir.display()),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tasks root has a symlink component",
            ),
        ));
    }
    match fs::symlink_metadata(tasks_dir) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(TasksError::io(
                format!("refusing non-directory tasks root {}", tasks_dir.display()),
                io::Error::new(io::ErrorKind::InvalidInput, "tasks root is not a directory"),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(TasksError::io(
                format!("reading tasks root {}", tasks_dir.display()),
                error,
            ));
        }
    }
    let dir = tasks_dir.join(status.as_str());
    match fs::symlink_metadata(&dir) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(TasksError::io(
                format!("refusing non-directory status path {}", dir.display()),
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "status path is not a directory",
                ),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(TasksError::io(format!("reading {}", dir.display()), error));
        }
    }
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(TasksError::io(format!("reading {}", dir.display()), e)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| TasksError::io(format!("reading {}", dir.display()), e))?;
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(Some(names))
}

/// `<root>/.plaesy/spec/<topic>/spec.md`.
#[must_use]
pub fn spec_doc(project_root: &Path, topic: &str) -> PathBuf {
    let topic = if validate_task_name(topic).is_ok() {
        topic
    } else {
        "__invalid_topic__"
    };
    project_root
        .join(".plaesy")
        .join(SPEC_DIR)
        .join(topic)
        .join(SPEC_FILE)
}

/// `<root>/.plaesy/plan/<topic>/plan.md`.
#[must_use]
pub fn plan_doc(project_root: &Path, topic: &str) -> PathBuf {
    let topic = if validate_task_name(topic).is_ok() {
        topic
    } else {
        "__invalid_topic__"
    };
    project_root
        .join(".plaesy")
        .join(PLAN_DIR)
        .join(topic)
        .join(PLAN_FILE)
}

/// Keeps the names that belong to `spec`. An empty spec keeps all of them.
pub(crate) fn for_spec(
    tasks_dir: &Path,
    status: Status,
    names: Vec<String>,
    spec: &str,
) -> Vec<String> {
    if spec.is_empty() {
        return names;
    }
    names
        .into_iter()
        .filter(|name| spec_of(tasks_dir, status, name) == spec)
        .collect()
}

/// Finds the status folder that holds `task`, searching in lifecycle order.
#[must_use]
pub fn find_task(tasks_dir: &Path, task: &str) -> Option<(Status, PathBuf)> {
    if validate_task_name(task).is_err() {
        return None;
    }
    STATUSES.into_iter().find_map(|status| {
        let path = tasks_dir.join(status.as_str()).join(task);
        (!path_has_symlink_component(&path) && path.is_dir()).then_some((status, path))
    })
}

/// Every task of the project, ordered by status, then by name.
///
/// # Errors
///
/// I/O failures while listing a status folder.
pub fn list_all_tasks(tasks_dir: &Path) -> Result<Vec<TaskRef>, TasksError> {
    let mut refs = Vec::new();
    for status in STATUSES {
        for name in list_tasks(tasks_dir, status)?.unwrap_or_default() {
            let task_file = tasks_dir.join(status.as_str()).join(&name).join(TASK_FILE);
            match fs::symlink_metadata(&task_file) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(TasksError::io(
                        format!("refusing symlinked task file {}", task_file.display()),
                        io::Error::new(io::ErrorKind::InvalidInput, "task file is a symlink"),
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(TasksError::io(
                        format!("reading task file {}", task_file.display()),
                        error,
                    ));
                }
            }
            let fm = match read_task_frontmatter(tasks_dir, status, &name) {
                Ok(frontmatter) => frontmatter,
                Err(error) if error.kind() == io::ErrorKind::NotFound => HashMap::new(),
                Err(error) => {
                    return Err(TasksError::io(
                        format!("reading task file {}", task_file.display()),
                        error,
                    ));
                }
            };
            refs.push(TaskRef {
                spec: fm.get("spec").cloned().unwrap_or_default(),
                status,
                title: fm.get("title").cloned().unwrap_or_default(),
                phase: fm.get("phase").cloned().unwrap_or_default(),
                name,
            });
        }
    }
    Ok(refs)
}

/// Reports inconsistencies: a folder without `task.md`, a frontmatter status that
/// differs from its folder, a missing status, and a name that appears in two
/// status folders.
///
/// # Errors
///
/// I/O failures while listing a status folder.
pub fn check_integrity(tasks_dir: &Path) -> Result<Vec<Problem>, TasksError> {
    let mut problems = Vec::new();
    let mut seen: BTreeMap<String, Status> = BTreeMap::new();
    for status in STATUSES {
        for name in list_tasks(tasks_dir, status)?.unwrap_or_default() {
            if let Some(prev) = seen.get(&name) {
                problems.push(Problem {
                    task: name.clone(),
                    message: format!("in both {prev} and {status}"),
                });
            } else {
                seen.insert(name.clone(), status);
            }
            let doc = tasks_dir.join(status.as_str()).join(&name).join(TASK_FILE);
            match fs::symlink_metadata(&doc) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    problems.push(Problem {
                        task: name,
                        message: "task.md is a symlink".into(),
                    });
                    continue;
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    problems.push(Problem {
                        task: name,
                        message: "no task.md in the task folder".into(),
                    });
                    continue;
                }
                Err(error) => {
                    return Err(TasksError::io(
                        format!("reading task file {}", doc.display()),
                        error,
                    ));
                }
            }
            if !doc.is_file() {
                problems.push(Problem {
                    task: name,
                    message: "task.md is not a regular file".into(),
                });
                continue;
            }
            let frontmatter = match read_task_frontmatter(tasks_dir, status, &name) {
                Ok(frontmatter) => frontmatter,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    problems.push(Problem {
                        task: name,
                        message: "no task.md in the task folder".into(),
                    });
                    continue;
                }
                Err(error) => {
                    return Err(TasksError::io(
                        format!("reading task file {}", doc.display()),
                        error,
                    ));
                }
            };
            if frontmatter
                .get("topic")
                .is_none_or(|topic| validate_task_name(topic).is_err())
            {
                problems.push(Problem {
                    task: name.clone(),
                    message: "no topic in task.md; every task needs one".into(),
                });
            }
            match frontmatter.get("status") {
                None => problems.push(Problem {
                    task: name,
                    message: format!("no status in frontmatter, folder is {status}"),
                }),
                Some(declared) if declared != status.as_str() => problems.push(Problem {
                    task: name,
                    message: format!("frontmatter status {declared}, folder is {status}"),
                }),
                Some(_) => {}
            }
        }
    }
    Ok(problems)
}

#[cfg(test)]
#[path = "tasks_test.rs"]
mod tasks_test;
