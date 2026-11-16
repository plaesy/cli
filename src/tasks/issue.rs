//! Structured issue metadata for Plaesy's Markdown-backed task tracker.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use serde::{Deserialize, Serialize};

use super::{list_tasks, read_frontmatter, validate_task_name, Status, STATUSES, TASK_FILE};

/// Exclusive inter-process guard for transactions that can change the issue graph.
///
/// The lock file is persistent and must not be removed or replaced while processes
/// may be running. Advisory-lock semantics require cooperating processes and a
/// filesystem that implements OS file locks consistently.
pub struct IssueGraphLock {
    _file: File,
    tasks_root: PathBuf,
}

impl IssueGraphLock {
    /// Acquires the collection-scoped issue graph lock, waiting for its current owner.
    pub fn acquire(tasks_dir: &Path) -> Result<Self, String> {
        Self::acquire_inner(tasks_dir, false)?
            .ok_or_else(|| "issue graph lock unexpectedly busy".into())
    }

    /// Attempts to acquire the collection-scoped issue graph lock without waiting.
    pub fn try_acquire(tasks_dir: &Path) -> Result<Option<Self>, String> {
        Self::acquire_inner(tasks_dir, true)
    }

    fn acquire_inner(tasks_dir: &Path, nonblocking: bool) -> Result<Option<Self>, String> {
        let tasks_dir = absolute_path(tasks_dir)?;
        reject_symlink_or_non_directory_components(&tasks_dir)?;
        fs::create_dir_all(&tasks_dir).map_err(|error| {
            format!(
                "creating issue graph lock directory {}: {error}",
                tasks_dir.display()
            )
        })?;
        reject_symlink_or_non_directory_components(&tasks_dir)?;
        let tasks_root = fs::canonicalize(&tasks_dir).map_err(|error| {
            format!(
                "resolving issue graph lock directory {}: {error}",
                tasks_dir.display()
            )
        })?;
        let path = tasks_root.join(".issue-graph.lock");
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).write(true);
        #[cfg(unix)]
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        let file = options
            .open(&path)
            .map_err(|error| format!("opening issue graph lock {}: {error}", path.display()))?;
        if nonblocking {
            match file.try_lock() {
                Ok(()) => Ok(Some(Self {
                    _file: file,
                    tasks_root: tasks_root.clone(),
                })),
                Err(std::fs::TryLockError::WouldBlock) => Ok(None),
                Err(std::fs::TryLockError::Error(error)) => {
                    Err(format!("locking issue graph {}: {error}", path.display()))
                }
            }
        } else {
            file.lock()
                .map(|()| {
                    Some(Self {
                        _file: file,
                        tasks_root,
                    })
                })
                .map_err(|error| format!("locking issue graph {}: {error}", path.display()))
        }
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("resolving path {}: {error}", path.display()))
    }
}

fn reject_symlink_or_non_directory_components(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "refusing symlink path component {}",
                    current.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "refusing non-directory path component {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "checking path component {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

/// Stable identifier and metadata for an issue, independent of its lifecycle folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    /// Stable task identifier, normally the task folder name.
    pub id: String,
    /// Current lifecycle folder. Used to disambiguate legacy duplicate names.
    #[serde(skip, default = "default_status")]
    pub status: Status,
    /// Classification such as issue, story, bug, epic, task, or subtask.
    #[serde(default = "default_type", rename = "type")]
    pub kind: String,
    /// Optional parent issue identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Issues that must be completed before this one can proceed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Free-form labels for filtering and grouping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Optional owner, represented as a stable handle or name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// Optional milestone name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
}

fn default_type() -> String {
    "task".into()
}

fn default_status() -> Status {
    Status::Backlog
}

impl Issue {
    /// Read metadata from a task's optional `issue.json`; legacy folders become basic issues.
    pub fn read(tasks_dir: &Path, status: Status, name: &str) -> Result<Self, String> {
        validate_task_name(name).map_err(|error| error.to_string())?;
        let path = tasks_dir.join(status.as_str()).join(name);
        let metadata = path.join("issue.json");
        if let Some(bytes) = read_metadata_bytes(&path)? {
            let mut issue: Self = serde_json::from_slice(&bytes)
                .map_err(|e| format!("parsing {}: {e}", metadata.display()))?;
            // The task directory is the canonical stable ID across lifecycle moves.
            // Treat an empty legacy ID as the folder name, but reject conflicting JSON IDs.
            if issue.id.is_empty() {
                issue.id = name.to_owned();
            } else if issue.id != name {
                return Err(format!(
                    "issue id {:?} does not match task folder {name}",
                    issue.id
                ));
            }
            issue.status = status;
            return Ok(issue);
        }

        let frontmatter = read_frontmatter(&path.join(TASK_FILE));
        Ok(Self {
            id: name.to_owned(),
            status,
            kind: frontmatter
                .get("type")
                .cloned()
                .unwrap_or_else(default_type),
            parent: None,
            depends_on: Vec::new(),
            labels: Vec::new(),
            assignee: None,
            milestone: None,
        })
    }

    /// Validate all references and hierarchy/dependency graph invariants for a task collection.
    pub fn validate_collection(tasks_dir: &Path) -> Result<Vec<String>, String> {
        let records = Self::load_collection(tasks_dir)?;
        let ids: BTreeSet<_> = records.keys().cloned().collect();
        let mut problems = Vec::new();
        for (id, issue) in &records {
            if let Some(parent) = &issue.parent {
                if !ids.contains(parent) {
                    problems.push(format!("{id}: missing parent {parent}"));
                } else if parent == id {
                    problems.push(format!("{id}: cannot be its own parent"));
                }
            }
            if !["issue", "story", "bug", "epic", "task", "subtask"].contains(&issue.kind.as_str())
            {
                problems.push(format!("{id}: unknown type {:?}", issue.kind));
            }
            let unique_labels: BTreeSet<_> = issue.labels.iter().collect();
            if unique_labels.len() != issue.labels.len() {
                problems.push(format!("{id}: duplicate labels"));
            }
            let unique_dependencies: BTreeSet<_> = issue.depends_on.iter().collect();
            if unique_dependencies.len() != issue.depends_on.len() {
                problems.push(format!("{id}: duplicate dependencies"));
            }
            for dependency in &issue.depends_on {
                if !ids.contains(dependency) {
                    problems.push(format!("{id}: missing dependency {dependency}"));
                } else if dependency == id {
                    problems.push(format!("{id}: cannot depend on itself"));
                }
            }
        }
        problems.extend(find_cycles(&records, |issue| {
            issue.parent.iter().cloned().collect()
        }));
        problems.extend(find_cycles(&records, |issue| issue.depends_on.clone()));
        Ok(problems)
    }

    /// Load every issue keyed by stable ID, including legacy tasks without metadata files.
    pub fn load_collection(tasks_dir: &Path) -> Result<BTreeMap<String, Issue>, String> {
        let mut records: BTreeMap<String, Issue> = BTreeMap::new();
        for status in STATUSES {
            for name in list_tasks(tasks_dir, status)
                .map_err(|e| e.to_string())?
                .unwrap_or_default()
            {
                let issue = Self::read(tasks_dir, status, &name)?;
                if issue.id != name {
                    return Err(format!(
                        "issue id {:?} does not match task folder {name}",
                        issue.id
                    ));
                }
                if let Some(existing) = records.get(&issue.id) {
                    return Err(format!(
                        "duplicate issue id {:?}: tasks exist in both {} and {}",
                        issue.id, existing.status, status
                    ));
                }
                records.insert(issue.id.clone(), issue);
            }
        }
        Ok(records)
    }
}

fn find_cycles<F>(records: &BTreeMap<String, Issue>, edges: F) -> Vec<String>
where
    F: Fn(&Issue) -> Vec<String>,
{
    let mut problems = BTreeSet::new();
    // 0 = unseen, 1 = active on the current DFS path, 2 = fully processed.
    let mut colors: BTreeMap<String, u8> = BTreeMap::new();

    // Keep one frame per active node so nodes remain active until every child
    // has been visited. This avoids recursion for untrusted, very deep graphs.
    for id in records.keys() {
        if colors.get(id) == Some(&2) {
            continue;
        }
        colors.insert(id.clone(), 1);
        let mut stack = vec![(id.clone(), edges(&records[id]), 0usize)];

        while let Some((current, neighbors, next_index)) = stack.last_mut() {
            if *next_index == neighbors.len() {
                let finished = current.clone();
                stack.pop();
                colors.insert(finished, 2);
                continue;
            }

            let next = neighbors[*next_index].clone();
            *next_index += 1;
            if !records.contains_key(&next) {
                continue;
            }
            match colors.get(&next).copied() {
                Some(1) => {
                    problems.insert(format!("{next}: relationship cycle detected"));
                }
                Some(2) => {}
                _ => {
                    colors.insert(next.clone(), 1);
                    stack.push((next.clone(), edges(&records[&next]), 0));
                }
            }
        }
    }
    problems.into_iter().collect()
}

pub fn read_metadata_bytes(task_dir: &Path) -> Result<Option<Vec<u8>>, String> {
    use std::io::Read;

    let metadata = task_dir.join("issue.json");
    let directory = crate::memory::fs_safety::DirHandle::open_nofollow(task_dir)
        .map_err(|error| format!("opening task directory {}: {error}", task_dir.display()))?;
    let mut file = match directory.open_file_read_nofollow(Path::new("issue.json")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("opening {}: {error}", metadata.display())),
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("reading {}: {error}", metadata.display()))?;
    Ok(Some(bytes))
}

/// Writes metadata while the caller holds the collection's issue-graph lock.
impl IssueGraphLock {
    /// Writes metadata only within the collection protected by this guard.
    pub fn write_metadata(&self, task_dir: &Path, issue: &Issue) -> Result<(), String> {
        let task_dir = absolute_path(task_dir)?;
        let lexical_root = absolute_path(&self.tasks_root)?;
        if !task_dir.starts_with(&lexical_root) || task_dir == lexical_root {
            return Err(format!(
                "task directory {} is outside the locked collection {}",
                task_dir.display(),
                lexical_root.display()
            ));
        }
        let canonical_root = fs::canonicalize(&self.tasks_root).map_err(|error| {
            format!(
                "resolving collection {}: {error}",
                self.tasks_root.display()
            )
        })?;
        let canonical_task = fs::canonicalize(&task_dir)
            .map_err(|error| format!("resolving task directory {}: {error}", task_dir.display()))?;
        if !canonical_task.starts_with(&canonical_root) || canonical_task == canonical_root {
            return Err(format!(
                "task directory {} resolves outside locked collection {}",
                task_dir.display(),
                canonical_root.display()
            ));
        }
        write_metadata_unlocked_inner(&task_dir, issue)
    }
}

/// Raw metadata writer used only by unit-test fixtures.
#[cfg(test)]
pub(crate) fn write_metadata_unlocked(task_dir: &Path, issue: &Issue) -> Result<(), String> {
    write_metadata_unlocked_inner(task_dir, issue)
}

fn write_metadata_unlocked_inner(task_dir: &Path, issue: &Issue) -> Result<(), String> {
    use std::io::Write;

    let path = task_dir.join("issue.json");
    let text = serde_json::to_string_pretty(issue).map_err(|e| e.to_string())? + "\n";
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    #[cfg(target_os = "linux")]
    let directory = crate::memory::fs_safety::DirHandle::open_nofollow(task_dir)
        .map_err(|error| format!("opening task directory {}: {error}", task_dir.display()))?;
    let (temporary_name, mut file) = (0..100)
        .find_map(|_| {
            let sequence = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let name = format!(".issue.json.tmp-{}-{sequence}", std::process::id());
            #[cfg(target_os = "linux")]
            let result = directory.create_new_file(Path::new(&name));
            #[cfg(not(target_os = "linux"))]
            let result = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(task_dir.join(&name));
            match result {
                Ok(file) => Some(Ok((name, file))),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .unwrap_or_else(|| {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "could not allocate a unique issue metadata temporary file",
            ))
        })
        .map_err(|error| format!("writing {}: {error}", path.display()))?;
    #[cfg(not(target_os = "linux"))]
    let temporary = task_dir.join(&temporary_name);
    let result = (|| -> std::io::Result<()> {
        file.write_all(text.as_bytes())?;
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.file_type().is_file() {
                file.set_permissions(metadata.permissions())?;
            }
        }
        file.sync_all()?;
        drop(file);
        #[cfg(target_os = "linux")]
        directory.replace_within(Path::new(&temporary_name), Path::new("issue.json"))?;
        #[cfg(not(target_os = "linux"))]
        fs::rename(&temporary, &path)?;
        #[cfg(target_os = "linux")]
        directory.sync()?;
        #[cfg(not(target_os = "linux"))]
        if let Ok(directory) = fs::File::open(task_dir) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        #[cfg(target_os = "linux")]
        let _ = directory.remove_file(Path::new(&temporary_name));
        #[cfg(not(target_os = "linux"))]
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("writing {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::tasks_dir;

    #[cfg(target_os = "linux")]
    #[test]
    fn guarded_metadata_write_does_not_follow_task_path_replacement() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(root.path());
        let task = tasks.join("todo/item");
        fs::create_dir_all(&task).unwrap();
        let outside = root.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let sentinel = outside.join("issue.json");
        fs::write(&sentinel, b"external sentinel").unwrap();
        let guard = IssueGraphLock::acquire(&tasks).unwrap();

        fs::rename(&task, tasks.join("todo/item-original")).unwrap();
        symlink(&outside, &task).unwrap();

        let result = guard.write_metadata(&task, &issue("item", None, &[]));

        assert!(result.is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"external sentinel");
    }

    #[test]
    fn issue_graph_lock_serializes_independent_threads() {
        use std::sync::{mpsc, Arc, Barrier};
        use std::thread;

        let root = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(root.path());
        fs::create_dir_all(&tasks).unwrap();
        let tasks = Arc::new(tasks);
        let entered = Arc::new(Barrier::new(2));
        let (release_tx, release_rx) = mpsc::channel();
        let thread_tasks = Arc::clone(&tasks);
        let thread_entered = Arc::clone(&entered);
        let worker = thread::spawn(move || {
            let _guard = IssueGraphLock::acquire(&thread_tasks).unwrap();
            thread_entered.wait();
            release_rx.recv().unwrap();
        });

        entered.wait();
        assert!(IssueGraphLock::try_acquire(&tasks).unwrap().is_none());
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        assert!(IssueGraphLock::try_acquire(&tasks).unwrap().is_some());
    }

    #[test]
    fn graph_lock_rejects_metadata_write_to_another_collection() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let first_tasks = tasks_dir(first.path());
        let second_tasks = tasks_dir(second.path());
        let foreign_task = second_tasks.join("todo/foreign");
        fs::create_dir_all(&foreign_task).unwrap();
        let guard = IssueGraphLock::acquire(&first_tasks).unwrap();

        let result = guard.write_metadata(&foreign_task, &issue("foreign", None, &[]));

        assert!(result.is_err());
        assert!(!foreign_task.join("issue.json").exists());
    }

    fn issue(id: &str, parent: Option<&str>, depends_on: &[&str]) -> Issue {
        Issue {
            id: id.into(),
            status: Status::Todo,
            kind: "task".into(),
            parent: parent.map(str::to_owned),
            depends_on: depends_on.iter().map(|s| (*s).into()).collect(),
            labels: vec![],
            assignee: None,
            milestone: None,
        }
    }

    #[test]
    fn cycle_detection_handles_deep_acyclic_graph_without_recursion() {
        let mut records = BTreeMap::new();
        let count = 50_000;
        for i in 0..count {
            let parent = (i + 1 < count).then(|| format!("node-{:05}", i + 1));
            records.insert(
                format!("node-{i:05}"),
                issue(&format!("node-{i:05}"), parent.as_deref(), &[]),
            );
        }
        assert!(find_cycles(&records, |item| item.parent.iter().cloned().collect()).is_empty());

        let mut cyclic_records = records;
        cyclic_records.get_mut("node-49999").unwrap().parent = Some("node-00000".into());
        let cycle_problems = find_cycles(&cyclic_records, |item| {
            item.parent.iter().cloned().collect()
        });
        assert!(
            !cycle_problems.is_empty(),
            "cycle not found: {}",
            cycle_problems.len()
        );
    }

    #[test]
    fn cycle_detection_handles_shared_children_self_edges_and_disconnected_nodes() {
        let mut records = BTreeMap::new();
        records.insert("a".into(), issue("a", None, &[]));
        records.insert("b".into(), issue("b", None, &[]));
        records.insert("c".into(), issue("c", None, &[]));
        records.insert("d".into(), issue("d", None, &[]));

        records.get_mut("a").unwrap().depends_on = vec!["b".into(), "c".into()];
        records.get_mut("b").unwrap().depends_on = vec!["d".into()];
        records.get_mut("c").unwrap().depends_on = vec!["d".into()];
        assert!(find_cycles(&records, |item| item.depends_on.clone()).is_empty());

        records.get_mut("d").unwrap().depends_on = vec!["d".into()];
        let problems = find_cycles(&records, |item| item.depends_on.clone());
        assert!(problems
            .iter()
            .any(|problem| problem.contains("relationship cycle")));
    }

    #[test]
    fn metadata_write_never_overwrites_a_colliding_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir
            .path()
            .join(format!(".issue.json.tmp-{}-0", std::process::id()));
        fs::write(&candidate, "do not overwrite").unwrap();

        write_metadata_unlocked(dir.path(), &issue("item", None, &[])).unwrap();

        assert_eq!(fs::read_to_string(candidate).unwrap(), "do not overwrite");
        let written: Issue =
            serde_json::from_slice(&fs::read(dir.path().join("issue.json")).unwrap()).unwrap();
        assert_eq!(written.id, "item");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn metadata_write_rejects_symlinked_task_directory() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), root.path().join("task-link")).unwrap();

        let result =
            write_metadata_unlocked(&root.path().join("task-link"), &issue("item", None, &[]));

        assert!(result.is_err());
        assert!(!outside.path().join("issue.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn metadata_write_replaces_symlink_without_modifying_target() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("outside.json");
        let link = dir.path().join("issue.json");
        fs::write(&target, "outside sentinel").unwrap();
        symlink(&target, &link).unwrap();

        write_metadata_unlocked(dir.path(), &issue("item", None, &[])).unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "outside sentinel");
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_file());
    }

    #[cfg(unix)]
    #[test]
    fn metadata_write_preserves_existing_file_permissions() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("issue.json");
        fs::write(&path, "old metadata").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

        write_metadata_unlocked(dir.path(), &issue("item", None, &[])).unwrap();

        assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn metadata_snapshot_rejects_symlinked_issue_json() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let task = root.path().join("task");
        fs::create_dir_all(&task).unwrap();
        let outside = root.path().join("outside.json");
        fs::write(&outside, "{\\\"secret\\\":true}").unwrap();
        symlink(&outside, task.join("issue.json")).unwrap();

        let error = read_metadata_bytes(&task).unwrap_err();
        assert!(error.contains("issue.json"));
        assert_eq!(
            fs::read_to_string(outside).unwrap(),
            "{\\\"secret\\\":true}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn issue_read_rejects_symlinked_metadata() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let task = root.path().join("todo/task");
        fs::create_dir_all(&task).unwrap();
        let outside = root.path().join("outside.json");
        fs::write(
            &outside,
            serde_json::to_vec(&issue("task", None, &[])).unwrap(),
        )
        .unwrap();
        symlink(&outside, task.join("issue.json")).unwrap();

        let error = Issue::read(root.path(), Status::Todo, "task").unwrap_err();
        assert!(error.contains("opening") && error.contains("issue.json"));
    }

    #[test]
    fn reads_legacy_tasks_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("todo").join("high_legacy");
        fs::create_dir_all(&task).unwrap();
        fs::write(task.join(TASK_FILE), "---\ntitle: Legacy\n---\n").unwrap();
        let parsed = Issue::read(dir.path(), Status::Todo, "high_legacy").unwrap();
        assert_eq!(parsed.kind, "task");
        assert_eq!(parsed.id, "high_legacy");
    }

    #[test]
    fn task_move_preserves_issue_id_and_relationships_without_cascade() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(dir.path());
        for status in STATUSES {
            fs::create_dir_all(tasks.join(status.as_str())).unwrap();
        }
        for (id, parent) in [("parent", None), ("child", Some("parent"))] {
            let folder = tasks.join("todo").join(id);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join(TASK_FILE), "---\ntitle: Task\n---\n").unwrap();
            write_metadata_unlocked(&folder, &issue(id, parent, &[])).unwrap();
        }

        super::super::move_task(&tasks, "parent", Status::Todo, Status::Doing).unwrap();
        let collection = Issue::load_collection(&tasks).unwrap();
        assert_eq!(collection["parent"].id, "parent");
        assert_eq!(collection["child"].parent.as_deref(), Some("parent"));
        assert!(Issue::validate_collection(&tasks).unwrap().is_empty());
    }

    #[test]
    fn duplicate_folder_names_across_statuses_report_both_statuses() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(dir.path());
        for status in STATUSES {
            fs::create_dir_all(tasks.join(status.as_str()).join("duplicate")).unwrap();
            fs::write(
                tasks
                    .join(status.as_str())
                    .join("duplicate")
                    .join(TASK_FILE),
                "---\ntitle: Duplicate\n---\n",
            )
            .unwrap();
        }
        let error = Issue::load_collection(&tasks).unwrap_err();
        assert!(error.contains("duplicate issue id \"duplicate\""));
        assert!(error.contains("backlog") && error.contains("todo"));
    }

    #[test]
    fn issue_read_rejects_path_traversal_name() {
        let dir = tempfile::tempdir().unwrap();
        let error = Issue::read(dir.path(), Status::Todo, "../outside").unwrap_err();
        assert!(error.contains("invalid task slug"));
    }

    #[test]
    fn rejects_metadata_id_that_differs_from_folder_name() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("todo").join("canonical");
        fs::create_dir_all(&task).unwrap();
        fs::write(task.join(TASK_FILE), "---\ntitle: Task\n---\n").unwrap();
        write_metadata_unlocked(&task, &issue("forged", None, &[])).unwrap();

        let error = Issue::read(dir.path(), Status::Todo, "canonical").unwrap_err();
        assert!(error.contains("does not match task folder canonical"));
    }

    #[test]
    fn rejects_unknown_types_and_duplicate_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(dir.path());
        fs::create_dir_all(tasks.join("todo")).unwrap();
        let folder = tasks.join("todo").join("item");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(TASK_FILE), "---\\ntitle: Item\\n---\\n").unwrap();
        let mut record = issue("item", None, &["other", "other"]);
        record.kind = "unknown".into();
        record.labels = vec!["tag".into(), "tag".into()];
        write_metadata_unlocked(&folder, &record).unwrap();
        let problems = Issue::validate_collection(&tasks).unwrap();
        assert!(problems.iter().any(|p| p.contains("unknown type")));
        assert!(problems.iter().any(|p| p.contains("duplicate labels")));
        assert!(problems
            .iter()
            .any(|p| p.contains("duplicate dependencies")));
        assert!(problems
            .iter()
            .any(|p| p.contains("missing dependency other")));
    }

    #[test]
    fn detects_missing_references_and_cycles() {
        let dir = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(dir.path());
        for status in STATUSES {
            fs::create_dir_all(tasks.join(status.as_str())).unwrap();
        }
        for (id, issue) in [
            ("a", issue("a", Some("b"), &["missing"])),
            ("b", issue("b", Some("a"), &[])),
        ] {
            let folder = tasks.join("todo").join(id);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join(TASK_FILE), "---\ntitle: x\n---\n").unwrap();
            write_metadata_unlocked(&folder, &issue).unwrap();
        }
        let problems = Issue::validate_collection(&tasks).unwrap();
        assert!(problems
            .iter()
            .any(|p| p.contains("missing dependency missing")));
        assert!(problems.iter().any(|p| p.contains("cycle detected")));
        assert!(problems
            .iter()
            .any(|p| p.contains("a: relationship cycle detected")));
    }
}
