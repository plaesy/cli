//! Durable lifecycle markers for notes that an owner explicitly forgets.
//!
//! The event and tombstones live in the vault (outside the indexer's hidden-directory walk), so
//! they follow the notes across synced devices. A tombstone is written before unlinking the note;
//! if a process stops between those steps, indexing still fails closed and a later forget can
//! finish the removal.

use crate::memory::error::{MemoryError, Result};
use crate::memory::fs_safety::DirHandle;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const STATE_DIR: &str = ".plaesy-memory";
const TOMBSTONES_DIR: &str = "tombstones";
const EVENTS_DIR: &str = "events";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LifecycleEvent {
    schema: u32,
    event_id: String,
    operation: String,
    phase: String,
    occurred_at: String,
    trigger: String,
    target_id: Option<String>,
    path_sha256: String,
    content_sha256: Option<String>,
    scope: Vec<String>,
    from_state: String,
    to_state: String,
    evidence: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ForgetIntent {
    event_id: String,
}

fn sha256(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn io_error(path: &Path, source: std::io::Error) -> MemoryError {
    MemoryError::IoError {
        path: path.display().to_string(),
        source,
    }
}

fn child_dir(parent: &Path, name: &str, create: bool) -> Result<Option<PathBuf>> {
    let path = parent.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_dir() => {}
        Ok(_) => {
            return Err(MemoryError::ConfigError(format!(
                "lifecycle state path is not a directory: {}",
                path.display()
            )))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => {
            match std::fs::create_dir(&path) {
                Ok(()) => {}
                Err(create_error) if create_error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(create_error) => return Err(io_error(&path, create_error)),
            }
            let meta = std::fs::symlink_metadata(&path).map_err(|e| io_error(&path, e))?;
            if !meta.file_type().is_dir() {
                return Err(MemoryError::ConfigError(format!(
                    "lifecycle state path is not a directory: {}",
                    path.display()
                )));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_error(&path, e)),
    }
    let real_path = std::fs::canonicalize(&path).map_err(|e| io_error(&path, e))?;
    if !real_path.starts_with(parent) {
        return Err(MemoryError::ConfigError(format!(
            "lifecycle state path escapes its parent: {}",
            real_path.display()
        )));
    }
    Ok(Some(real_path))
}

fn ensure_state_dir(root: &Path, name: &str) -> Result<(PathBuf, DirHandle)> {
    let root = std::fs::canonicalize(root).map_err(|e| io_error(root, e))?;
    let state_root = child_dir(&root, STATE_DIR, true)?.ok_or_else(|| {
        MemoryError::ConfigError("could not create lifecycle state directory".into())
    })?;
    let real_dir = child_dir(&state_root, name, true)?
        .ok_or_else(|| MemoryError::ConfigError("could not create lifecycle directory".into()))?;
    let handle = DirHandle::open_nofollow(&real_dir).map_err(|e| io_error(&real_dir, e))?;
    Ok((real_dir, handle))
}

fn existing_state_dir(root: &Path, name: &str) -> Result<Option<PathBuf>> {
    let root = std::fs::canonicalize(root).map_err(|e| io_error(root, e))?;
    let Some(state_root) = child_dir(&root, STATE_DIR, false)? else {
        return Ok(None);
    };
    child_dir(&state_root, name, false)
}

fn write_new_json(
    dir: &Path,
    handle: &DirHandle,
    name: &str,
    value: &impl Serialize,
) -> Result<()> {
    let path = dir.join(name);
    let mut file = handle
        .create_new_file(Path::new(name))
        .map_err(|e| io_error(&path, e))?;
    serde_json::to_writer(&mut file, value)
        .map_err(|e| MemoryError::SerializationError(e.to_string()))?;
    file.write_all(b"\n").map_err(|e| io_error(&path, e))?;
    file.sync_all().map_err(|e| io_error(&path, e))?;
    handle.sync().map_err(|e| io_error(dir, e))
}

fn tombstone_names(relative_path: &str, note_id: Option<&str>) -> Vec<String> {
    let path_key = format!("path-{}", sha256(relative_path));
    let mut keys = vec![path_key];
    if let Some(id) = note_id.filter(|id| !id.trim().is_empty()) {
        keys.push(format!("id-{}", sha256(id.trim())));
    }
    keys
}

/// Whether this note path or stable note id has an owner-authored forget tombstone.
pub(crate) fn is_forgotten(
    root: &Path,
    relative_path: &str,
    note_id: Option<&str>,
) -> Result<bool> {
    if id_is_forgotten(root, note_id)? {
        return Ok(true);
    }
    let Some(real_dir) = existing_state_dir(root, TOMBSTONES_DIR)? else {
        return Ok(false);
    };
    is_marked(
        &real_dir,
        vec![format!("path-{}.json", sha256(relative_path))],
    )
}

/// Whether an explicit tombstone exists for a stable note identity.
pub(crate) fn id_is_forgotten(root: &Path, note_id: Option<&str>) -> Result<bool> {
    let Some(id) = note_id.filter(|id| !id.trim().is_empty()) else {
        return Ok(false);
    };
    let Some(real_dir) = existing_state_dir(root, TOMBSTONES_DIR)? else {
        return Ok(false);
    };
    is_marked(&real_dir, vec![format!("id-{}.json", sha256(id.trim()))])
}

/// Whether every stable identity currently carrying a forgotten note's content was forgotten.
pub(crate) fn content_is_forgotten(root: &Path, content: &str) -> Result<bool> {
    let Some(real_dir) = existing_state_dir(root, TOMBSTONES_DIR)? else {
        return Ok(false);
    };
    is_marked(&real_dir, vec![format!("content-{}.json", sha256(content))])
}

fn is_marked(real_dir: &Path, keys: Vec<String>) -> Result<bool> {
    for key in keys {
        let marker = real_dir.join(key);
        match std::fs::symlink_metadata(&marker) {
            Ok(meta) if meta.file_type().is_file() => return Ok(true),
            Ok(_) => {
                return Err(MemoryError::ConfigError(format!(
                    "lifecycle tombstone is not a regular file: {}",
                    marker.display()
                )))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error(&marker, e)),
        }
    }
    Ok(false)
}

/// Persist an intent and tombstone before a forgotten note is removed from the vault.
pub(crate) fn begin_forget(
    root: &Path,
    relative_path: &str,
    note_id: Option<&str>,
    content: Option<&str>,
    scope: Vec<String>,
    reason: &str,
) -> Result<ForgetIntent> {
    let evidence = match reason {
        "owner-request" => "owner-request",
        "duplicate" => "duplicate-note",
        "obsolete" => "obsolete-note",
        "privacy" => "privacy-removal",
        _ => {
            return Err(MemoryError::ConfigError(
                "forget reason must be owner-request, duplicate, obsolete, or privacy".into(),
            ));
        }
    };
    let path_sha256 = sha256(relative_path);
    let event_id = Uuid::new_v4().to_string();
    let event = LifecycleEvent {
        schema: 1,
        event_id: event_id.clone(),
        operation: "forget".into(),
        phase: "intent".into(),
        occurred_at: Utc::now().to_rfc3339(),
        trigger: "plaesy memory forget".into(),
        target_id: note_id.map(str::to_owned),
        path_sha256: path_sha256.clone(),
        content_sha256: content.map(sha256),
        scope,
        from_state: "active".into(),
        to_state: "forgotten".into(),
        evidence: evidence.into(),
    };
    let (events_dir, events_handle) = ensure_state_dir(root, EVENTS_DIR)?;
    write_new_json(
        &events_dir,
        &events_handle,
        &format!("{event_id}.intent.json"),
        &event,
    )?;

    let (tombstones_dir, tombstones_handle) = ensure_state_dir(root, TOMBSTONES_DIR)?;
    let mut keys = tombstone_names(relative_path, note_id);
    if let Some(content) = content {
        keys.push(format!("content-{}", sha256(content)));
    }
    for key in keys {
        let marker = format!("{key}.json");
        let path = tombstones_dir.join(&marker);
        match write_new_json(&tombstones_dir, &tombstones_handle, &marker, &event) {
            Ok(()) => {}
            Err(MemoryError::IoError { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let meta = std::fs::symlink_metadata(&path).map_err(|e| io_error(&path, e))?;
        if !meta.file_type().is_file() {
            return Err(MemoryError::ConfigError(format!(
                "lifecycle tombstone is not a regular file: {}",
                path.display()
            )));
        }
    }
    Ok(ForgetIntent { event_id })
}

/// Record that an intended forget reached the complete state.
pub(crate) fn complete_forget(root: &Path, intent: ForgetIntent) -> Result<()> {
    let event = LifecycleEvent {
        schema: 1,
        event_id: intent.event_id.clone(),
        operation: "forget".into(),
        phase: "complete".into(),
        occurred_at: Utc::now().to_rfc3339(),
        trigger: "plaesy memory forget".into(),
        target_id: None,
        path_sha256: String::new(),
        content_sha256: None,
        scope: Vec::new(),
        from_state: "active".into(),
        to_state: "forgotten".into(),
        evidence: "source and index removal completed".into(),
    };
    let (dir, handle) = ensure_state_dir(root, EVENTS_DIR)?;
    write_new_json(
        &dir,
        &handle,
        &format!("{}.complete.json", intent.event_id),
        &event,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forget_writes_a_synced_tombstone_and_structured_audit_without_note_content() {
        let root = tempfile::tempdir().expect("vault");
        let intent = begin_forget(
            root.path(),
            "notes/private.md",
            Some("note-uuid"),
            Some("private note body"),
            vec!["project-a".into()],
            "owner-request",
        )
        .expect("intent");

        assert!(is_forgotten(root.path(), "notes/private.md", Some("note-uuid")).unwrap());
        assert!(is_forgotten(root.path(), "notes/renamed.md", Some("note-uuid")).unwrap());
        assert!(is_forgotten(root.path(), "notes/private.md", None).unwrap());

        complete_forget(root.path(), intent).expect("completion event");
        let events = root.path().join(STATE_DIR).join(EVENTS_DIR);
        let entries: Vec<_> = std::fs::read_dir(events)
            .expect("event directory")
            .collect::<std::io::Result<_>>()
            .expect("event entries");
        assert_eq!(entries.len(), 2);
        let intent_text = std::fs::read_to_string(
            entries
                .iter()
                .find(|entry| entry.file_name().to_string_lossy().ends_with("intent.json"))
                .expect("intent entry")
                .path(),
        )
        .expect("intent contents");
        assert!(intent_text.contains("owner-request"));
        assert!(intent_text.contains("project-a"));
        assert!(!intent_text.contains("private note body"));
        assert!(content_is_forgotten(root.path(), "private note body").unwrap());
    }

    #[test]
    fn no_tombstone_is_not_a_forget() {
        let root = tempfile::tempdir().expect("vault");
        assert!(!is_forgotten(root.path(), "notes/a.md", None).unwrap());
    }

    #[test]
    fn forget_requires_a_supported_reason_category() {
        let root = tempfile::tempdir().expect("vault");
        assert!(begin_forget(
            root.path(),
            "notes/a.md",
            None,
            None,
            Vec::new(),
            "not-a-category"
        )
        .is_err());
    }

    #[test]
    fn idless_content_tombstone_matches_exact_content_without_retaining_it() {
        let root = tempfile::tempdir().expect("vault");
        let original = "# Private\n\ncontent to remove\n";
        let intent = begin_forget(
            root.path(),
            "notes/private.md",
            None,
            Some(original),
            Vec::new(),
            "privacy",
        )
        .expect("intent");

        assert!(content_is_forgotten(root.path(), original).unwrap());
        assert!(!content_is_forgotten(root.path(), "unrelated content").unwrap());
        complete_forget(root.path(), intent).expect("completion event");
        let state = std::fs::read_to_string(
            root.path()
                .join(STATE_DIR)
                .join(TOMBSTONES_DIR)
                .join(format!("path-{}.json", sha256("notes/private.md"))),
        )
        .expect("tombstone");
        assert!(!state.contains("content to remove"));
    }
}
