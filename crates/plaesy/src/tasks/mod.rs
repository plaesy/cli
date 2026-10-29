//! Task lifecycle for a spec.
//!
//! A task is a folder:
//!
//! ```text
//! .plaesy/tasks/<status>/<priority>_<category>-<title>/
//!   spec.md          what the task is for
//!   contracts.json   the checklist that says when it is done
//! ```
//!
//! The status is the folder the task sits in. Moving a task moves its folder.
//! Port of `scripts/internal/tasks` (Go).

pub mod check;
pub mod claim;
pub mod contracts;
pub mod inflight;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use thiserror::Error;

/// Description file every task folder holds.
pub const TASK_FILE: &str = "spec.md";
/// Checklist file every task folder may hold.
pub const CONTRACT_FILE: &str = "contracts.json";
/// Plan file a task folder may hold.
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
/// [`TasksError::ProjectRootNotFound`] when no ancestor has one.
pub fn find_project_root(start: &Path) -> Result<PathBuf, TasksError> {
    start
        .ancestors()
        .find(|dir| dir.join(".plaesy").is_dir())
        .map(Path::to_path_buf)
        .ok_or(TasksError::ProjectRootNotFound)
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
            let value = value.trim().trim_matches(['"', '\'']);
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
    let dir = tasks_dir.join(status.as_str());
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

/// Spec a task belongs to, from its `spec.md` frontmatter.
#[must_use]
pub fn spec_of(tasks_dir: &Path, status: Status, task: &str) -> String {
    read_frontmatter(&tasks_dir.join(status.as_str()).join(task).join(TASK_FILE))
        .remove("spec")
        .unwrap_or_default()
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

/// Reports inconsistencies: a folder without `spec.md`, a frontmatter status that
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
                    message: "no spec.md in the task folder".into(),
                });
                continue;
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
title: {title}
phase: implement
status: backlog
---

## Goal

## Scope

## Context

## Description

## Decision

## KPI

## Verification

## Reference

## Dependencies

## Notes
";

/// Creates a task in `backlog` named `<priority>_<slug>`, with a `spec.md` and an
/// empty `contracts.json`. Returns the folder name.
///
/// # Errors
///
/// Invalid priority or slug, a name that already exists in any status, or I/O failures.
pub fn add_task(
    tasks_dir: &Path,
    spec: &str,
    priority: &str,
    slug: &str,
    title: &str,
) -> Result<String, TasksError> {
    validate_priority(priority)?;
    if slug.is_empty() || slug.contains(['/', '\\']) {
        return Err(TasksError::InvalidSlug(slug.to_owned()));
    }
    let name = format!("{priority}_{slug}");
    if find_task(tasks_dir, &name).is_some() {
        return Err(TasksError::Duplicate(name));
    }
    let dir = tasks_dir.join(Status::Backlog.as_str()).join(&name);
    fs::create_dir_all(&dir)
        .map_err(|e| TasksError::io(format!("creating {}", dir.display()), e))?;
    let body = TASK_TEMPLATE
        .replace("{spec}", spec)
        .replace("{title}", title);
    write_file(&dir.join(TASK_FILE), &body)?;
    let contract = format!("{{\n  \"id\": {name:?},\n  \"checklist\": []\n}}\n");
    write_file(&dir.join(CONTRACT_FILE), &contract)?;
    Ok(name)
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
    let source = tasks_dir.join(from.as_str()).join(task);
    let target = tasks_dir.join(to.as_str()).join(task);
    if !source.is_dir() {
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
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| TasksError::io(format!("creating {}", parent.display()), e))?;
    }
    fs::rename(&source, &target)
        .map_err(|e| TasksError::io(format!("moving {task} to {to}"), e))?;
    set_frontmatter_status(&target.join(TASK_FILE), to)
}

/// Rewrites the `status:` line in a task's frontmatter. A missing file or a file
/// without a status line is left unchanged.
fn set_frontmatter_status(path: &Path, status: Status) -> Result<(), TasksError> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(());
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
            return write_file(path, &lines.join("\n"));
        }
    }
    Ok(())
}

/// Moves the next backlog task of `spec` to `todo` and returns its name.
///
/// # Errors
///
/// Same as [`move_task`] and [`next_backlog_task`].
pub fn pop_next_task(tasks_dir: &Path, spec: &str) -> Result<Option<String>, TasksError> {
    let Some(task) = next_backlog_task(tasks_dir, spec)? else {
        return Ok(None);
    };
    move_task(tasks_dir, &task, Status::Backlog, Status::Todo)?;
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

/// `doing` → `blocked`, appending the reason to `spec.md`.
///
/// # Errors
///
/// As [`move_task`], plus I/O while appending the reason.
pub fn block_task(tasks_dir: &Path, task: &str, reason: &str) -> Result<(), TasksError> {
    let reason = if reason.is_empty() {
        "No reason provided"
    } else {
        reason
    };
    move_task_as(tasks_dir, "block", task, Status::Doing, Status::Blocked)?;
    let doc = tasks_dir.join("blocked").join(task).join(TASK_FILE);
    let mut text = fs::read_to_string(&doc).unwrap_or_default();
    text.push_str(&format!("\n## Blocked Reason\n{reason}\n"));
    write_file(&doc, &text)
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
        let text = fs::read_to_string(tasks.join("blocked/medium_y/spec.md")).unwrap();
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
