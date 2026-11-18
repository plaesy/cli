//! `movement` helpers split out of `mod.rs`.

use super::*;

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
    let _graph_lock = acquire_issue_graph_lock(tasks_dir)?;
    move_task_as_locked(tasks_dir, verb, task, from, to)
}

pub(super) fn acquire_issue_graph_lock(
    tasks_dir: &Path,
) -> Result<issue::IssueGraphLock, TasksError> {
    issue::IssueGraphLock::acquire(tasks_dir)
        .map_err(|error| TasksError::io("acquiring issue graph lock", io::Error::other(error)))
}

pub(super) fn write_file_atomically(
    task_dir: &crate::memory::fs_safety::DirHandle,
    destination: &str,
    contents: &[u8],
) -> io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut temp_name = None;
    let mut file = None;
    for _ in 0..128 {
        let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = format!(".{TASK_FILE}.{}.{}.tmp", std::process::id(), sequence);
        match task_dir.create_new_file(Path::new(&candidate)) {
            Ok(created) => {
                temp_name = Some(candidate);
                file = Some(created);
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    let temp_name = temp_name.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate task temp file",
        )
    })?;
    let write_result = (|| {
        let mut file = file.ok_or_else(|| io::Error::other("temp file was not created"))?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        task_dir.replace_within(Path::new(&temp_name), Path::new(destination))
    })();
    if write_result.is_err() {
        let _ = task_dir.remove_file(Path::new(&temp_name));
    }
    write_result
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
    let task_handle = crate::memory::fs_safety::DirHandle::open_nofollow(&source)
        .map_err(|e| TasksError::io(format!("opening {}", source.display()), e))?;
    let original = task_handle
        .open_file_read_relative_nofollow(Path::new(TASK_FILE))
        .and_then(|mut file| {
            use std::io::Read;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .map_err(|e| TasksError::io(format!("reading {}", task_file.display()), e))?;
    let updated = with_frontmatter_status(&original, to);
    // Replace atomically so a failed write cannot truncate the original task file.
    write_file_atomically(&task_handle, TASK_FILE, &updated)
        .map_err(|e| TasksError::io(format!("writing {}", task_file.display()), e))?;
    // Windows refuses to rename a directory while a handle to it is open, so
    // release the handle first and reopen it only if the move must be undone.
    drop(task_handle);
    if let Err(error) = rename_with_retry(&source, &target) {
        let restore = crate::memory::fs_safety::DirHandle::open_nofollow(&source)
            .and_then(|handle| write_file_atomically(&handle, TASK_FILE, &original));
        if let Err(restore_error) = restore {
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

/// Renames a task folder, retrying briefly on `PermissionDenied`. On Windows a
/// concurrent reader can hold a handle inside the folder for a moment, and the
/// rename is refused until that handle closes.
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    const MAX_RETRIES: u32 = 40;
    let mut delay = std::time::Duration::from_millis(1);
    let mut retries = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error)
                if error.kind() == io::ErrorKind::PermissionDenied && retries < MAX_RETRIES =>
            {
                retries += 1;
                std::thread::sleep(delay);
                delay = (delay * 2).min(std::time::Duration::from_millis(50));
            }
            Err(error) => return Err(error),
        }
    }
}

/// Rewrites the `status:` line in task frontmatter bytes. A missing file or a
/// file without a status line is left unchanged.
pub(super) fn with_frontmatter_status(original: &[u8], status: Status) -> Vec<u8> {
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
    let task_handle = crate::memory::fs_safety::DirHandle::open_nofollow(&task_dir)
        .map_err(|error| TasksError::io(format!("opening {}", task_dir.display()), error))?;
    let original = task_handle
        .open_file_read_relative_nofollow(Path::new(TASK_FILE))
        .and_then(|mut file| {
            use std::io::Read;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .map_err(|error| TasksError::io(format!("reading {}", doc.display()), error))?;
    let mut blocked = String::from_utf8(original.clone()).map_err(|error| {
        TasksError::io(
            format!("task file {} is not valid UTF-8", doc.display()),
            io::Error::new(io::ErrorKind::InvalidData, error),
        )
    })?;
    blocked.push_str(&format!("\n## Blocked Reason\n{reason}\n"));
    write_file_atomically(&task_handle, TASK_FILE, blocked.as_bytes())
        .map_err(|error| TasksError::io(format!("writing {}", doc.display()), error))?;
    if let Err(error) =
        move_task_as_locked(tasks_dir, "block", task, Status::Doing, Status::Blocked)
    {
        if let Err(restore_error) = write_file_atomically(&task_handle, TASK_FILE, &original) {
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
