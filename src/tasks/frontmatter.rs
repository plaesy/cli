//! `frontmatter` helpers split out of `mod.rs`.

use super::*;

/// Reads the top-level `key: value` pairs from a file's YAML frontmatter.
/// Returns an empty map when the file is absent or has no frontmatter.
#[must_use]
pub fn read_frontmatter(path: &Path) -> HashMap<String, String> {
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    read_frontmatter_from_text(&text)
}

pub(crate) fn read_frontmatter_from_text(text: &str) -> HashMap<String, String> {
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
            // A quoted scalar: the same quote character opens and closes the value.
            let value = if matches!(value.as_bytes(), [first @ (b'"' | b'\''), .., last] if first == last)
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

pub(crate) fn read_task_frontmatter(
    tasks_dir: &Path,
    status: Status,
    task: &str,
) -> Result<HashMap<String, String>, io::Error> {
    let root = crate::memory::fs_safety::DirHandle::open_nofollow(tasks_dir)?;
    read_task_frontmatter_from_handle(
        &root,
        &Path::new(status.as_str()).join(task).join(TASK_FILE),
    )
}

fn read_task_frontmatter_from_handle(
    root: &crate::memory::fs_safety::DirHandle,
    relative: &Path,
) -> Result<HashMap<String, String>, io::Error> {
    let mut file = root.open_file_read_relative_nofollow(relative)?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text)?;
    Ok(read_frontmatter_from_text(&text))
}

fn task_frontmatter(tasks_dir: &Path, status: Status, task: &str) -> HashMap<String, String> {
    if validate_task_name(task).is_err() {
        return HashMap::new();
    }
    let Some((found_status, _)) = find_task(tasks_dir, task) else {
        return HashMap::new();
    };
    if found_status != status {
        return HashMap::new();
    }
    read_task_frontmatter(tasks_dir, status, task).unwrap_or_default()
}

/// Spec a task belongs to, from its `task.md` frontmatter.
#[must_use]
pub fn spec_of(tasks_dir: &Path, status: Status, task: &str) -> String {
    task_frontmatter(tasks_dir, status, task)
        .remove("spec")
        .unwrap_or_default()
}

/// Topic a task belongs to, from its `topic:` frontmatter. `None` when the task has
/// no topic: such a task needs no spec and no plan.
#[must_use]
pub fn topic_of(tasks_dir: &Path, status: Status, task: &str) -> Option<String> {
    task_frontmatter(tasks_dir, status, task)
        .remove("topic")
        .filter(|topic| validate_task_name(topic).is_ok())
}

pub(super) fn path_has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return true;
        }
    }
    false
}
