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
    Done,
    Blocked,
}

/// All statuses, in lifecycle order.
pub const STATUSES: [Status; 5] = [
    Status::Backlog,
    Status::Todo,
    Status::Doing,
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

/// Reads the top-level `key: value` pairs from a file's YAML frontmatter.
/// Returns an empty map when the file is absent or has no frontmatter.
#[must_use]
pub fn read_frontmatter(path: &Path) -> HashMap<String, String> {
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let text = text.replace("\r\n", "\n");
    let mut lines = text.split('\n');
    if lines.next().map(str::trim) != Some("---") {
        return HashMap::new();
    }
    let mut out = HashMap::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if line.starts_with(' ') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            let value = if value.len() >= 2
                && matches!(value.as_bytes()[0], b'"' | b'\'')
                && value.as_bytes()[0] == *value.as_bytes().last().expect("non-empty value")
            {
                &value[1..value.len() - 1]
            } else {
                value
            };
            out.insert(key.trim().to_owned(), value.to_owned());
        }
    }
    out
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

/// Spec a task belongs to, from its `task.md` frontmatter.
#[must_use]
pub fn spec_of(tasks_dir: &Path, status: Status, task: &str) -> String {
    if validate_task_name(task).is_err() {
        return String::new();
    }
    read_frontmatter(&tasks_dir.join(status.as_str()).join(task).join(TASK_FILE))
        .remove("spec")
        .unwrap_or_default()
}

/// Topic a task belongs to, from its `topic:` frontmatter. `None` when the task has
/// no topic: such a task needs no spec and no plan.
#[must_use]
pub fn topic_of(tasks_dir: &Path, status: Status, task: &str) -> Option<String> {
    if validate_task_name(task).is_err() {
        return None;
    }
    read_frontmatter(&tasks_dir.join(status.as_str()).join(task).join(TASK_FILE))
        .remove("topic")
        .filter(|topic| validate_task_name(topic).is_ok())
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

fn path_has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return true;
        }
    }
    false
}

/// Finds the status folder that holds `task`, searching in lifecycle order.
#[must_use]
pub fn find_task(tasks_dir: &Path, task: &str) -> Option<(Status, PathBuf)> {
    if validate_task_name(task).is_err() {
        return None;
    }
    STATUSES.into_iter().find_map(|status| {
        let path = tasks_dir.join(status.as_str()).join(task);
        path.is_dir().then_some((status, path))
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
            let fm = read_frontmatter(&tasks_dir.join(status.as_str()).join(&name).join(TASK_FILE));
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
            if !doc.exists() {
                problems.push(Problem {
                    task: name,
                    message: "no task.md in the task folder".into(),
                });
                continue;
            }
            if topic_of(tasks_dir, status, &name).is_none() {
                problems.push(Problem {
                    task: name.clone(),
                    message: "no topic in task.md; every task needs one".into(),
                });
            }
            match read_frontmatter(&doc).get("status") {
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

const TASK_TEMPLATE: &str = "---
spec: {spec}
topic: {topic}
title: {title}
phase: implement
status: backlog
---

{title}
";

/// Topic slug from a task's category and title: `<category>-<title>` in kebab-case.
/// Only ASCII letters and digits are kept; every other run of characters becomes one
/// hyphen.
///
/// # Errors
///
/// [`TasksError::InvalidSlug`] when nothing is left after normalizing.
pub fn topic_slug(category: &str, title: &str) -> Result<String, TasksError> {
    let joined = format!("{category} {title}");
    let mut slug = String::new();
    for ch in joined.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        return Err(TasksError::InvalidSlug(joined));
    }
    Ok(slug)
}

/// Creates a task in `backlog` named `<priority>_<topic>` with a `task.md` and an
/// empty `contracts.json`. The task's topic is `topic`, and the topic holds its spec
/// and plan under `.plaesy/spec/` and `.plaesy/plan/`.
///
/// # Errors
///
/// Invalid priority or topic, a name that already exists in any status, or I/O failures.
pub fn add_task(
    tasks_dir: &Path,
    spec: &str,
    priority: &str,
    topic: &str,
    title: &str,
) -> Result<String, TasksError> {
    let _graph_lock = acquire_issue_graph_lock(tasks_dir)?;
    add_task_locked(tasks_dir, spec, priority, topic, title)
}

pub(crate) fn add_task_locked(
    tasks_dir: &Path,
    spec: &str,
    priority: &str,
    topic: &str,
    title: &str,
) -> Result<String, TasksError> {
    if path_has_symlink_component(tasks_dir)
        || fs::symlink_metadata(tasks_dir).is_ok_and(|metadata| !metadata.is_dir())
    {
        return Err(TasksError::io(
            format!(
                "refusing symlinked or non-directory tasks root {}",
                tasks_dir.display()
            ),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tasks root is not a real directory",
            ),
        ));
    }
    validate_priority(priority)?;
    if topic.is_empty() || topic.contains(['/', '\\']) {
        return Err(TasksError::InvalidSlug(topic.to_owned()));
    }
    let name = format!("{priority}_{topic}");
    if find_task(tasks_dir, &name).is_some() {
        return Err(TasksError::Duplicate(name));
    }
    let status_dir = tasks_dir.join(Status::Backlog.as_str());
    if fs::symlink_metadata(&status_dir).is_ok_and(|metadata| !metadata.is_dir()) {
        return Err(TasksError::io(
            format!(
                "refusing non-directory status path {}",
                status_dir.display()
            ),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "status path is not a directory",
            ),
        ));
    }
    let dir = status_dir.join(&name);
    fs::create_dir_all(&status_dir)
        .map_err(|error| TasksError::io(format!("creating {}", status_dir.display()), error))?;
    let status_metadata = fs::symlink_metadata(&status_dir);
    if !status_metadata.is_ok_and(|metadata| metadata.is_dir()) {
        return Err(TasksError::io(
            format!(
                "refusing non-directory status path {}",
                status_dir.display()
            ),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "status path is not a directory",
            ),
        ));
    }
    match fs::create_dir(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(TasksError::Duplicate(name));
        }
        Err(error) => {
            return Err(TasksError::io(format!("creating {}", dir.display()), error));
        }
    }
    let spec = single_line_frontmatter_value(spec);
    let topic = single_line_frontmatter_value(topic);
    let title = single_line_frontmatter_value(title);
    let body = TASK_TEMPLATE
        .replace("{spec}", &spec)
        .replace("{topic}", &topic)
        .replace("{title}", &title);
    let contract = format!("{{\n  \"id\": {name:?},\n  \"checklist\": []\n}}\n");
    if let Err(error) = write_task_files(&dir, &body, &contract) {
        if let Err(cleanup_error) = fs::remove_dir_all(&dir) {
            return Err(TasksError::io(
                format!(
                    "creating task failed ({error}); removing partial task failed: {cleanup_error}"
                ),
                io::Error::other(error),
            ));
        }
        return Err(error);
    }
    Ok(name)
}

fn write_task_files(dir: &Path, body: &str, contract: &str) -> Result<(), TasksError> {
    write_file(&dir.join(TASK_FILE), body)?;
    write_file(&dir.join(CONTRACT_FILE), contract)
}

fn single_line_frontmatter_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\r' | '\n' => ' ',
            _ => ch,
        })
        .collect()
}

fn write_file(path: &Path, contents: &str) -> Result<(), TasksError> {
    fs::write(path, contents).map_err(|e| TasksError::io(format!("writing {}", path.display()), e))
}

/// Most urgent task in `backlog` for `spec` (any spec when empty).
///
/// # Errors
///
/// I/O failures while listing the backlog.
pub fn next_backlog_task(tasks_dir: &Path, spec: &str) -> Result<Option<String>, TasksError> {
    let Some(names) = list_tasks(tasks_dir, Status::Backlog)? else {
        return Ok(None);
    };
    let mut names = for_spec(tasks_dir, Status::Backlog, names, spec);
    names.sort_by_key(|name| priority_rank(name));
    Ok(names.into_iter().next())
}

fn validate_task_name(task: &str) -> Result<(), TasksError> {
    let mut components = Path::new(task).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) if !task.is_empty() => Ok(()),
        _ => Err(TasksError::InvalidSlug(task.to_owned())),
    }
}

/// Moves one task folder from `from` to `to` and sets its frontmatter status.
///
/// # Errors
///
/// [`TasksError::NotFoundIn`] or [`TasksError::WrongStatus`] when the task is not
/// where the verb expects, [`TasksError::AlreadyExists`] at the target, or I/O.
pub fn move_task(tasks_dir: &Path, task: &str, from: Status, to: Status) -> Result<(), TasksError> {
    move_task_as(tasks_dir, "move", task, from, to)
}

fn move_task_as(
    tasks_dir: &Path,
    verb: &str,
    task: &str,
    from: Status,
    to: Status,
) -> Result<(), TasksError> {
    let _graph_lock = acquire_issue_graph_lock(tasks_dir)?;
    move_task_as_locked(tasks_dir, verb, task, from, to)
}

fn acquire_issue_graph_lock(tasks_dir: &Path) -> Result<issue::IssueGraphLock, TasksError> {
    issue::IssueGraphLock::acquire(tasks_dir)
        .map_err(|error| TasksError::io("acquiring issue graph lock", io::Error::other(error)))
}

fn move_task_as_locked(
    tasks_dir: &Path,
    verb: &str,
    task: &str,
    from: Status,
    to: Status,
) -> Result<(), TasksError> {
    validate_task_name(task)?;
    if path_has_symlink_component(tasks_dir) {
        return Err(TasksError::io(
            format!("refusing symlinked tasks root {}", tasks_dir.display()),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tasks root has a symlink component",
            ),
        ));
    }
    let source_status_dir = tasks_dir.join(from.as_str());
    let target_status_dir = tasks_dir.join(to.as_str());
    let source_status_metadata = fs::symlink_metadata(&source_status_dir);
    if source_status_metadata.is_ok_and(|metadata| !metadata.is_dir()) {
        return Err(TasksError::io(
            format!(
                "refusing non-directory status path {}",
                source_status_dir.display()
            ),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "status path is not a directory",
            ),
        ));
    }
    let source = source_status_dir.join(task);
    let target = target_status_dir.join(task);
    let source_metadata = fs::symlink_metadata(&source);
    if !source_metadata.is_ok_and(|metadata| metadata.is_dir()) {
        return Err(match find_task(tasks_dir, task) {
            Some((actual, _)) => TasksError::WrongStatus {
                task: task.to_owned(),
                actual,
                verb: verb.to_owned(),
                needed: from,
            },
            None => TasksError::NotFoundIn {
                status: from,
                task: task.to_owned(),
            },
        });
    }
    if target.exists() {
        return Err(TasksError::AlreadyExists {
            status: to,
            task: task.to_owned(),
        });
    }
    fs::create_dir_all(&target_status_dir)
        .map_err(|e| TasksError::io(format!("creating {}", target_status_dir.display()), e))?;
    let target_status_metadata = fs::symlink_metadata(&target_status_dir);
    if !target_status_metadata.is_ok_and(|metadata| metadata.is_dir()) {
        return Err(TasksError::io(
            format!(
                "refusing non-directory status path {}",
                target_status_dir.display()
            ),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "status path is not a directory",
            ),
        ));
    }
    let task_file = source.join(TASK_FILE);
    let file_metadata = fs::symlink_metadata(&task_file)
        .map_err(|e| TasksError::io(format!("reading {}", task_file.display()), e))?;
    if !file_metadata.is_file() {
        return Err(TasksError::io(
            format!("refusing non-regular task file {}", task_file.display()),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "task file is not a regular file",
            ),
        ));
    }
    let original = fs::read(&task_file)
        .map_err(|e| TasksError::io(format!("reading {}", task_file.display()), e))?;
    let updated = with_frontmatter_status(&original, to);
    // Persist the destination status before moving the directory. If writing fails,
    // the source task remains untouched; after rename the task is already consistent.
    fs::write(&task_file, &updated)
        .map_err(|e| TasksError::io(format!("writing {}", task_file.display()), e))?;
    if let Err(error) = fs::rename(&source, &target) {
        if let Err(restore_error) = fs::write(&task_file, &original) {
            return Err(TasksError::io(
                format!(
                    "moving {task} to {to} failed ({error}); restoring source task failed: {restore_error}"
                ),
                error,
            ));
        }
        return Err(TasksError::io(format!("moving {task} to {to}"), error));
    }
    Ok(())
}

/// Rewrites the `status:` line in task frontmatter bytes. A missing file or a
/// file without a status line is left unchanged.
fn with_frontmatter_status(original: &[u8], status: Status) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(original) else {
        return original.to_vec();
    };
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let mut in_front = false;
    for line in &mut lines {
        if line == "---" {
            if in_front {
                break;
            }
            in_front = true;
            continue;
        }
        if in_front && line.starts_with("status:") {
            *line = format!("status: {status}");
            return lines.join("\n").into_bytes();
        }
    }
    original.to_vec()
}

/// Moves the next backlog task of `spec` to `todo` and returns its name.
///
/// # Errors
///
/// Same as [`move_task`] and [`next_backlog_task`].
pub fn pop_next_task(tasks_dir: &Path, spec: &str) -> Result<Option<String>, TasksError> {
    let _graph_lock = acquire_issue_graph_lock(tasks_dir)?;
    let Some(task) = next_backlog_task(tasks_dir, spec)? else {
        return Ok(None);
    };
    move_task_as_locked(tasks_dir, "move", &task, Status::Backlog, Status::Todo)?;
    Ok(Some(task))
}

/// `todo` → `doing`.
///
/// # Errors
///
/// As [`move_task`], with the `start` verb in messages.
pub fn start_task(tasks_dir: &Path, task: &str) -> Result<(), TasksError> {
    move_task_as(tasks_dir, "start", task, Status::Todo, Status::Doing)
}

/// `doing` → `done`.
///
/// # Errors
///
/// As [`move_task`], with the `complete` verb in messages.
pub fn complete_task(tasks_dir: &Path, task: &str) -> Result<(), TasksError> {
    move_task_as(tasks_dir, "complete", task, Status::Doing, Status::Done)
}

/// `doing` → `blocked`, appending the reason to `task.md`.
///
/// # Errors
///
/// As [`move_task`], plus I/O while appending the reason.
pub fn block_task(tasks_dir: &Path, task: &str, reason: &str) -> Result<(), TasksError> {
    let _graph_lock = acquire_issue_graph_lock(tasks_dir)?;
    validate_task_name(task)?;
    if path_has_symlink_component(tasks_dir) {
        return Err(TasksError::io(
            format!("refusing symlinked tasks root {}", tasks_dir.display()),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tasks root has a symlink component",
            ),
        ));
    }
    let reason = if reason.is_empty() {
        "No reason provided"
    } else {
        reason
    };
    let doing_dir = tasks_dir.join("doing");
    let doing_metadata = fs::symlink_metadata(&doing_dir)
        .map_err(|error| TasksError::io(format!("reading {}", doing_dir.display()), error))?;
    if !doing_metadata.is_dir() {
        return Err(TasksError::io(
            format!("refusing non-directory status path {}", doing_dir.display()),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "status path is not a directory",
            ),
        ));
    }
    let task_dir = doing_dir.join(task);
    let task_dir_metadata = fs::symlink_metadata(&task_dir)
        .map_err(|error| TasksError::io(format!("reading {}", task_dir.display()), error))?;
    if !task_dir_metadata.is_dir() {
        return Err(TasksError::io(
            format!("refusing non-directory task path {}", task_dir.display()),
            io::Error::new(io::ErrorKind::InvalidInput, "task path is not a directory"),
        ));
    }
    let doc = task_dir.join(TASK_FILE);
    let metadata = fs::symlink_metadata(&doc)
        .map_err(|error| TasksError::io(format!("reading {}", doc.display()), error))?;
    if !metadata.is_file() {
        return Err(TasksError::io(
            format!("refusing non-regular task file {}", doc.display()),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "task file is not a regular file",
            ),
        ));
    }
    let original = fs::read(&doc)
        .map_err(|error| TasksError::io(format!("reading {}", doc.display()), error))?;
    let mut blocked = String::from_utf8(original.clone()).map_err(|error| {
        TasksError::io(
            format!("task file {} is not valid UTF-8", doc.display()),
            io::Error::new(io::ErrorKind::InvalidData, error),
        )
    })?;
    blocked.push_str(&format!("\n## Blocked Reason\n{reason}\n"));
    write_file(&doc, &blocked)?;
    if let Err(error) =
        move_task_as_locked(tasks_dir, "block", task, Status::Doing, Status::Blocked)
    {
        if let Err(restore_error) = fs::write(&doc, original) {
            return Err(TasksError::io(
                format!(
                    "blocking {task} failed ({error}); restoring task file failed: {restore_error}"
                ),
                io::Error::other(error),
            ));
        }
        return Err(error);
    }
    Ok(())
}

/// `blocked` → `todo`.
///
/// # Errors
///
/// As [`move_task`], with the `unblock` verb in messages.
pub fn unblock_task(tasks_dir: &Path, task: &str) -> Result<(), TasksError> {
    move_task_as(tasks_dir, "unblock", task, Status::Blocked, Status::Todo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        let tasks = tasks_dir(dir.path());
        (dir, tasks)
    }

    #[test]
    fn status_round_trips_and_rejects_unknown() {
        for s in STATUSES {
            assert_eq!(s.as_str().parse::<Status>().unwrap(), s);
        }
        assert!(matches!(
            "later".parse::<Status>(),
            Err(TasksError::InvalidStatus(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn list_tasks_rejects_symlinked_ancestor_of_tasks_root() {
        use std::os::unix::fs::symlink;

        let (root, tasks) = project();
        let external = root.path().join("external");
        let alias = root.path().join("alias");
        fs::create_dir_all(external.join("tasks/backlog")).unwrap();
        fs::write(external.join("tasks/backlog/low_external"), "").unwrap();
        symlink(&external, &alias).unwrap();

        let result = list_tasks(&alias.join("tasks"), Status::Backlog);

        assert!(external.join("tasks/backlog/low_external").exists());
        assert!(result.is_err());
        assert_eq!(list_tasks(&tasks, Status::Backlog).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn list_tasks_rejects_symlinked_tasks_root() {
        use std::os::unix::fs::symlink;

        let (_root, tasks) = project();
        let external = tasks.parent().unwrap().join("external-tasks");
        fs::create_dir_all(external.join("backlog")).unwrap();
        symlink(&external, &tasks).unwrap();

        assert!(list_tasks(&tasks, Status::Backlog).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn list_tasks_rejects_symlinked_status_directory() {
        use std::os::unix::fs::symlink;

        let (_root, tasks) = project();
        let backlog = tasks.join("backlog");
        fs::create_dir_all(&backlog).unwrap();
        let external = tasks.parent().unwrap().join("external-backlog");
        fs::rename(&backlog, &external).unwrap();
        symlink(&external, &backlog).unwrap();

        assert!(list_tasks(&tasks, Status::Backlog).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn add_task_does_not_create_through_symlinked_ancestor_of_tasks_root() {
        use std::os::unix::fs::symlink;

        let (root, tasks) = project();
        let external = root.path().join("external");
        let alias = root.path().join("alias");
        fs::create_dir_all(external.join("tasks/backlog")).unwrap();
        symlink(&external, &alias).unwrap();

        let result = add_task(&alias.join("tasks"), "spec", "low", "linked", "Linked");

        assert!(result.is_err());
        assert!(!external.join("tasks/backlog/low_linked").exists());
        assert!(!tasks.join("backlog/low_linked").exists());
    }

    #[cfg(unix)]
    #[test]
    fn add_task_does_not_create_through_symlinked_tasks_root() {
        use std::os::unix::fs::symlink;

        let (_root, tasks) = project();
        let external = tasks.parent().unwrap().join("external-tasks");
        fs::create_dir_all(external.join("backlog")).unwrap();
        symlink(&external, &tasks).unwrap();

        let result = add_task(&tasks, "spec", "low", "linked", "Linked");

        assert!(result.is_err());
        assert!(!external.join("backlog/low_linked").exists());
    }

    #[cfg(unix)]
    #[test]
    fn add_task_does_not_create_through_symlinked_status_directory() {
        use std::os::unix::fs::symlink;

        let (_root, tasks) = project();
        let backlog = tasks.join("backlog");
        fs::create_dir_all(&backlog).unwrap();
        let external = tasks.parent().unwrap().join("external-backlog");
        fs::rename(&backlog, &external).unwrap();
        symlink(&external, &backlog).unwrap();

        let result = add_task(&tasks, "spec", "low", "linked", "Linked");

        assert!(result.is_err());
        assert!(!external.join("low_linked").exists());
    }

    #[test]
    fn topic_of_rejects_path_traversal_from_frontmatter() {
        let (_dir, tasks) = project();
        let name = add_task(&tasks, "s", "low", "safe", "Safe").unwrap();
        let task_file = tasks.join("backlog").join(&name).join(TASK_FILE);
        fs::write(&task_file, "---\nspec: s\ntopic: ../../outside\n---\n").unwrap();
        assert_eq!(topic_of(&tasks, Status::Backlog, &name), None);
    }

    #[test]
    fn spec_and_plan_doc_keep_invalid_topics_inside_plaesy() {
        let (root, _tasks) = project();
        let outside = root.path().join("outside");

        let spec = spec_doc(root.path(), "../../outside");
        let plan = plan_doc(root.path(), "../outside");

        assert!(spec.starts_with(root.path().join(".plaesy/spec")));
        assert!(plan.starts_with(root.path().join(".plaesy/plan")));
        assert!(!spec.starts_with(&outside));
        assert!(!plan.starts_with(&outside));
        assert!(spec.ends_with("__invalid_topic__/spec.md"));
        assert!(plan.ends_with("__invalid_topic__/plan.md"));
    }

    #[test]
    fn read_frontmatter_does_not_strip_unmatched_quotes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.md");
        fs::write(&path, "---\ntitle: \"keep opening quote\n---\n").unwrap();
        assert_eq!(read_frontmatter(&path)["title"], "\"keep opening quote");
    }

    #[test]
    fn read_frontmatter_uses_last_colon_in_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task.md");
        fs::write(&path, "---\ntitle: https://example.test/a:b\n---\n").unwrap();
        assert_eq!(read_frontmatter(&path)["title"], "https://example.test/a:b");
    }

    #[test]
    fn task_frontmatter_values_are_preserved_as_strings() {
        let (_dir, tasks) = project();
        let name = add_task(&tasks, "# prod", "low", "safe", "# note").unwrap();
        let frontmatter = read_frontmatter(&tasks.join("backlog").join(name).join(TASK_FILE));
        assert_eq!(frontmatter["spec"], "# prod");
        assert_eq!(frontmatter["title"], "# note");
    }

    #[test]
    fn task_title_cannot_inject_frontmatter_fields() {
        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "low", "safe", "Title\nspec: injected").unwrap();
        let frontmatter = read_frontmatter(&tasks.join("backlog").join(name).join(TASK_FILE));
        assert_eq!(frontmatter["spec"], "auth");
    }

    #[test]
    fn topic_slug_joins_category_and_title_in_kebab_case() {
        assert_eq!(
            topic_slug("Feature", "Login form!!").unwrap(),
            "feature-login-form"
        );
        assert!(matches!(
            topic_slug("---", "  "),
            Err(TasksError::InvalidSlug(_))
        ));
    }

    #[test]
    fn priority_rank_orders_by_prefix() {
        assert!(priority_rank("critical_x") < priority_rank("low_x"));
        assert_eq!(priority_rank("unknown_x"), PRIORITIES.len());
    }

    #[test]
    fn frontmatter_reads_top_level_keys_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.md");
        fs::write(
            &path,
            "---\r\nspec: \"auth\"\n  nested: no\ntitle: 'Login'\n---\nstatus: ignored\n",
        )
        .unwrap();
        let fm = read_frontmatter(&path);
        assert_eq!(fm["spec"], "auth");
        assert_eq!(fm["title"], "Login");
        assert!(!fm.contains_key("nested"));
        assert!(!fm.contains_key("status"));
    }

    #[test]
    fn status_frontmatter_is_prepared_without_mutating_source_bytes() {
        let original = b"---\ntitle: Example\nstatus: todo\n---\nbody\n";
        let updated = with_frontmatter_status(original, Status::Doing);
        assert_eq!(original, b"---\ntitle: Example\nstatus: todo\n---\nbody\n");
        assert_eq!(updated, b"---\ntitle: Example\nstatus: doing\n---\nbody\n");
    }

    #[cfg(unix)]
    #[test]
    fn moving_task_does_not_write_through_symlinked_task_directory() {
        use std::os::unix::fs::symlink;

        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "low", "symlink-dir", "Symlink").unwrap();
        let task_dir = tasks.join("backlog").join(&name);
        let external = tasks.parent().unwrap().join("external-task-dir");
        fs::rename(&task_dir, &external).unwrap();
        symlink(&external, &task_dir).unwrap();
        let task_file = external.join(TASK_FILE);
        let original = fs::read(&task_file).unwrap();

        let result = move_task(&tasks, &name, Status::Backlog, Status::Todo);

        assert!(result.is_err());
        assert_eq!(fs::read(&task_file).unwrap(), original);
        assert!(task_dir
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn moving_task_rejects_symlinked_ancestor_of_tasks_root() {
        use std::os::unix::fs::symlink;

        let (root, _tasks) = project();
        let external = root.path().join("external");
        let alias = root.path().join("alias");
        fs::create_dir_all(external.join("tasks/backlog/low_external")).unwrap();
        fs::write(
            external.join("tasks/backlog/low_external/task.md"),
            "---\nstatus: backlog\n---\n",
        )
        .unwrap();
        symlink(&external, &alias).unwrap();

        let result = move_task(
            &alias.join("tasks"),
            "low_external",
            Status::Backlog,
            Status::Todo,
        );

        assert!(result.is_err());
        assert!(external.join("tasks/backlog/low_external").exists());
        assert!(!external.join("tasks/todo/low_external").exists());
    }

    #[cfg(unix)]
    #[test]
    fn moving_task_does_not_follow_symlinked_status_directory() {
        use std::os::unix::fs::symlink;

        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "low", "status-link", "Status link").unwrap();
        let backlog = tasks.join("backlog");
        let external = tasks.parent().unwrap().join("external-backlog");
        fs::rename(&backlog, &external).unwrap();
        symlink(&external, &backlog).unwrap();
        let source_task = external.join(&name);
        let original = fs::read(source_task.join(TASK_FILE)).unwrap();

        let result = move_task(&tasks, &name, Status::Backlog, Status::Todo);

        assert!(result.is_err());
        assert_eq!(fs::read(source_task.join(TASK_FILE)).unwrap(), original);
        assert!(backlog.symlink_metadata().unwrap().file_type().is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn moving_task_does_not_write_through_symlinked_task_file() {
        use std::os::unix::fs::symlink;

        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "low", "symlink", "Symlink").unwrap();
        move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
        let task_file = tasks.join("todo").join(&name).join(TASK_FILE);
        fs::remove_file(&task_file).unwrap();
        let external = tasks.parent().unwrap().join("external-task.md");
        let sentinel = b"external sentinel\n";
        fs::write(&external, sentinel).unwrap();
        symlink(&external, &task_file).unwrap();

        let result = start_task(&tasks, &name);

        assert!(result.is_err());
        assert_eq!(fs::read(&external).unwrap(), sentinel);
        assert!(task_file
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn block_task_rejects_symlinked_ancestor_of_tasks_root() {
        use std::os::unix::fs::symlink;

        let (root, _tasks) = project();
        let external = root.path().join("external");
        let alias = root.path().join("alias");
        let task_dir = external.join("tasks/doing/low_external");
        fs::create_dir_all(&task_dir).unwrap();
        let task_file = task_dir.join(TASK_FILE);
        let original = b"---\nstatus: doing\n---\nTask body\n";
        fs::write(&task_file, original).unwrap();
        symlink(&external, &alias).unwrap();

        let result = block_task(&alias.join("tasks"), "low_external", "reason");

        assert!(result.is_err());
        assert_eq!(fs::read(task_file).unwrap(), original);
        assert!(external.join("tasks/doing/low_external").exists());
        assert!(!external.join("tasks/blocked/low_external").exists());
    }

    #[cfg(unix)]
    #[test]
    fn block_task_does_not_write_through_symlinked_task_directory() {
        use std::os::unix::fs::symlink;

        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "low", "symlink-dir", "Symlink").unwrap();
        move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
        start_task(&tasks, &name).unwrap();
        let task_dir = tasks.join("doing").join(&name);
        let external = tasks.parent().unwrap().join("external-doing-task");
        fs::rename(&task_dir, &external).unwrap();
        symlink(&external, &task_dir).unwrap();
        let task_file = external.join(TASK_FILE);
        let original = fs::read(&task_file).unwrap();

        let result = block_task(&tasks, &name, "external reason");

        assert!(result.is_err());
        assert_eq!(fs::read(&task_file).unwrap(), original);
        assert!(task_dir
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn block_task_does_not_write_through_symlinked_task_file() {
        use std::os::unix::fs::symlink;

        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "low", "symlink-block", "Symlink").unwrap();
        move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
        start_task(&tasks, &name).unwrap();
        let task_file = tasks.join("doing").join(&name).join(TASK_FILE);
        fs::remove_file(&task_file).unwrap();
        let external = tasks.parent().unwrap().join("external-block.md");
        let sentinel = b"external sentinel\n";
        fs::write(&external, sentinel).unwrap();
        symlink(&external, &task_file).unwrap();

        let result = block_task(&tasks, &name, "external reason");

        assert!(result.is_err());
        assert_eq!(fs::read(&external).unwrap(), sentinel);
        assert!(task_file
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn lifecycle_commands_reject_path_traversal_names() {
        let (_dir, tasks) = project();
        add_task(&tasks, "auth", "medium", "safe", "Safe").unwrap();
        for result in [
            move_task(&tasks, "../outside", Status::Backlog, Status::Todo),
            block_task(&tasks, "../outside", "reason"),
            unblock_task(&tasks, "../outside"),
        ] {
            assert!(matches!(result, Err(TasksError::InvalidSlug(_))));
        }
        assert!(!tasks.join("../outside/task.md").exists());
    }

    #[test]
    fn add_then_start_complete_moves_the_folder_and_status() {
        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "high", "login-form", "Login form").unwrap();
        assert_eq!(name, "high_login-form");
        assert_eq!(
            pop_next_task(&tasks, "auth").unwrap().as_deref(),
            Some("high_login-form")
        );
        start_task(&tasks, &name).unwrap();
        let (status, _) = find_task(&tasks, &name).unwrap();
        assert_eq!(status, Status::Doing);
        complete_task(&tasks, &name).unwrap();
        let (status, path) = find_task(&tasks, &name).unwrap();
        assert_eq!(status, Status::Done);
        assert!(read_frontmatter(&path.join(TASK_FILE))["status"] == "done");
    }

    #[test]
    fn block_failure_keeps_original_task_content_in_source_status() {
        let (_dir, tasks) = project();
        let name = add_task(&tasks, "auth", "medium", "rollback", "Rollback").unwrap();
        move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
        start_task(&tasks, &name).unwrap();
        let source = tasks.join("doing").join(&name);
        let original = fs::read(source.join(TASK_FILE)).unwrap();
        fs::create_dir_all(tasks.join("blocked").join(&name)).unwrap();

        assert!(block_task(&tasks, &name, "cannot move").is_err());
        assert_eq!(fs::read(source.join(TASK_FILE)).unwrap(), original);
        assert!(source.exists());
    }

    #[test]
    fn wrong_status_names_where_the_task_is() {
        let (_dir, tasks) = project();
        add_task(&tasks, "auth", "low", "x", "X").unwrap();
        let err = start_task(&tasks, "low_x").unwrap_err();
        assert_eq!(err.to_string(), "low_x is in backlog; start needs todo");
    }

    #[test]
    fn block_appends_reason_and_unblock_returns_to_todo() {
        let (_dir, tasks) = project();
        add_task(&tasks, "auth", "medium", "y", "Y").unwrap();
        move_task(&tasks, "medium_y", Status::Backlog, Status::Todo).unwrap();
        start_task(&tasks, "medium_y").unwrap();
        block_task(&tasks, "medium_y", "waiting on API").unwrap();
        let text = fs::read_to_string(tasks.join("blocked/medium_y/task.md")).unwrap();
        assert!(text.contains("## Blocked Reason\nwaiting on API"));
        unblock_task(&tasks, "medium_y").unwrap();
        assert_eq!(find_task(&tasks, "medium_y").unwrap().0, Status::Todo);
    }

    #[test]
    fn next_task_prefers_higher_priority_and_respects_spec() {
        let (_dir, tasks) = project();
        add_task(&tasks, "auth", "low", "a", "A").unwrap();
        add_task(&tasks, "auth", "critical", "b", "B").unwrap();
        add_task(&tasks, "billing", "critical", "c", "C").unwrap();
        assert_eq!(
            next_backlog_task(&tasks, "auth").unwrap().as_deref(),
            Some("critical_b")
        );
        assert_eq!(
            next_backlog_task(&tasks, "billing").unwrap().as_deref(),
            Some("critical_c")
        );
    }

    #[test]
    fn integrity_flags_status_mismatch_and_duplicates() {
        let (_dir, tasks) = project();
        add_task(&tasks, "auth", "low", "z", "Z").unwrap();
        fs::create_dir_all(tasks.join("todo/low_z")).unwrap();
        let problems = check_integrity(&tasks).unwrap();
        assert!(problems
            .iter()
            .any(|p| p.message == "in both backlog and todo"));
    }

    #[test]
    fn concurrent_add_task_creates_one_folder_without_overwriting() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        for round in 0..25 {
            let root = tempfile::tempdir().unwrap();
            let tasks = tasks_dir(root.path());
            let backlog = tasks.join(Status::Backlog.as_str());
            fs::create_dir_all(&backlog).unwrap();
            let tasks = Arc::new(tasks);
            let barrier = Arc::new(Barrier::new(8));
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let tasks = Arc::clone(&tasks);
                    let barrier = Arc::clone(&barrier);
                    thread::spawn(move || {
                        barrier.wait();
                        add_task(&tasks, "spec", "high", "feature-same", "Same title")
                    })
                })
                .collect();
            let results: Vec<_> = handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();

            assert_eq!(
                results.iter().filter(|result| result.is_ok()).count(),
                1,
                "round {round}"
            );
            assert_eq!(
                results.iter().filter(|result| result.is_err()).count(),
                7,
                "round {round}"
            );
            let task_dir = backlog.join("high_feature-same");
            assert!(task_dir.join(TASK_FILE).is_file(), "round {round}");
            assert!(task_dir.join(CONTRACT_FILE).is_file(), "round {round}");
        }
    }

    #[test]
    fn invalid_slug_and_duplicate_are_rejected() {
        let (_dir, tasks) = project();
        assert!(matches!(
            add_task(&tasks, "s", "low", "a/b", "t"),
            Err(TasksError::InvalidSlug(_))
        ));
        add_task(&tasks, "s", "low", "dup", "t").unwrap();
        assert!(matches!(
            add_task(&tasks, "s", "low", "dup", "t"),
            Err(TasksError::Duplicate(_))
        ));
    }
}
