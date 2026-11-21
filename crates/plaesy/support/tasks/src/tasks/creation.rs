//! `creation` helpers split out of `mod.rs`.

use super::*;

const TASK_TEMPLATE: &str = r#"---
spec: {spec}
topic: {topic}
title: {title}
phase: implement
status: backlog
---

# {title}

## Goal
Describe the observable outcome this task should achieve.

## Scope
- Describe the work included.

## Non-goals
- Describe what is intentionally excluded.

## Deliverables
- List the expected outputs.

## Dependencies
- None, or list prerequisites and owners.

## Acceptance criteria
- Add observable criteria and link each to evidence in `contracts.json` where automatable.
"#;

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

pub fn add_task_locked(
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

pub(super) fn write_task_files(dir: &Path, body: &str, contract: &str) -> Result<(), TasksError> {
    let handle = crate::memory::fs_safety::DirHandle::open_nofollow(dir)
        .map_err(|error| TasksError::io(format!("opening {}", dir.display()), error))?;
    write_file_atomically(&handle, TASK_FILE, body.as_bytes()).map_err(|error| {
        TasksError::io(format!("writing {}", dir.join(TASK_FILE).display()), error)
    })?;
    if let Err(error) = write_file_atomically(&handle, CONTRACT_FILE, contract.as_bytes()) {
        if let Err(cleanup_error) = handle.remove_file(Path::new(TASK_FILE)) {
            return Err(TasksError::io(
                format!("writing contract failed ({error}); removing partial task file failed: {cleanup_error}"),
                io::Error::other(error),
            ));
        }
        return Err(TasksError::io(
            format!("writing {}", dir.join(CONTRACT_FILE).display()),
            error,
        ));
    }
    Ok(())
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

pub(super) fn validate_task_name(task: &str) -> Result<(), TasksError> {
    let mut components = Path::new(task).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) if !task.is_empty() => Ok(()),
        _ => Err(TasksError::InvalidSlug(task.to_owned())),
    }
}
