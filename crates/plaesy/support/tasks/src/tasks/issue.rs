//! Structured issue metadata for Plaesy's Markdown-backed task tracker.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use serde::{Deserialize, Serialize};

use super::{
    list_tasks, read_frontmatter_from_text, validate_task_name, Status, STATUSES, TASK_FILE,
};

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
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("resolving path {}: {error}", path.display()))?
    };
    Ok(strip_verbatim_prefix(absolute))
}

/// On Windows, `fs::canonicalize` returns `\\?\C:\...` paths. Those never compare equal to
/// ordinary paths with `starts_with`, so the lexical containment check must see the plain form.
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return path;
    }
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}

fn reject_symlink_or_non_directory_components(path: &Path) -> Result<(), String> {
    let prefixes = super::frontmatter::checked_prefixes(path)
        .map_err(|error| format!("checking path {}: {error}", path.display()))?;
    for current in prefixes {
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

/// Longest accepted label, counted in characters.
pub const MAX_LABEL_CHARS: usize = 32;

/// Reasons a label is rejected. Labels are lowercase, space-free tokens.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    /// The label is the empty string.
    #[error("label cannot be empty")]
    Empty,
    /// The label has more than 32 characters.
    #[error("label {0:?} is longer than 32 characters")]
    TooLong(String),
    /// The label contains a character outside lowercase ASCII letters, digits, `-` and `_`.
    #[error("label {0:?} may contain only lowercase letters, digits, '-' and '_' (no spaces)")]
    InvalidCharacter(String),
}

/// Checks one label: non-empty, at most [`MAX_LABEL_CHARS`] characters, and only
/// `a-z`, `0-9`, `-` or `_`. Labels are never normalized; a rejected label must be fixed by the caller.
pub fn validate_label(label: &str) -> Result<(), LabelError> {
    if label.is_empty() {
        return Err(LabelError::Empty);
    }
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(LabelError::TooLong(label.to_owned()));
    }
    let allowed = label
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if !allowed {
        return Err(LabelError::InvalidCharacter(label.to_owned()));
    }
    Ok(())
}

fn default_type() -> String {
    "task".into()
}

fn default_status() -> Status {
    Status::Backlog
}

impl Issue {
    /// Adds a validated label, keeping insertion order. Returns `false` if it was already present.
    pub fn add_label(&mut self, label: &str) -> Result<bool, LabelError> {
        validate_label(label)?;
        if self
            .labels
            .iter()
            .any(|existing| existing.as_str() == label)
        {
            return Ok(false);
        }
        self.labels.push(label.to_owned());
        Ok(true)
    }

    /// Removes a validated label. Returns `false` if the issue did not carry it.
    pub fn remove_label(&mut self, label: &str) -> Result<bool, LabelError> {
        validate_label(label)?;
        let before = self.labels.len();
        self.labels.retain(|existing| existing.as_str() != label);
        Ok(self.labels.len() != before)
    }

    /// True when the issue carries every label in `labels`. An empty filter matches everything.
    pub fn has_labels(&self, labels: &[String]) -> bool {
        labels.iter().all(|label| self.labels.contains(label))
    }

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

        let task_file = path.join(TASK_FILE);
        let relative = Path::new(status.as_str()).join(name).join(TASK_FILE);
        let root_handle = crate::memory::fs_safety::DirHandle::open_nofollow(tasks_dir)
            .map_err(|error| format!("opening tasks directory safely: {error}"))?;
        let mut task_file_handle = root_handle
            .open_file_read_relative_nofollow(&relative)
            .map_err(|error| format!("reading {}: {error}", task_file.display()))?;
        let mut task_contents = String::new();
        std::io::Read::read_to_string(&mut task_file_handle, &mut task_contents)
            .map_err(|error| format!("reading {}: {error}", task_file.display()))?;
        let frontmatter = read_frontmatter_from_text(&task_contents);
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
            for label in &issue.labels {
                if let Err(error) = validate_label(label) {
                    problems.push(format!("{id}: invalid label: {error}"));
                }
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
        // Compare canonical forms only. A lexical `starts_with` fails when the same folder is
        // spelled with a Windows 8.3 short name (`RUNNER~1`) in one path and its long name in another.
        let task_dir = absolute_path(task_dir)?;
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

/// Raw metadata writer for test fixtures. It takes no lock: do not use it in product code.
#[cfg(any(test, feature = "test-support"))]
pub fn write_metadata_unlocked(task_dir: &Path, issue: &Issue) -> Result<(), String> {
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
#[path = "issue_test.rs"]
mod issue_test;
