//! Unit tests for `tests` split out of `lifecycle.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn history_returns_recent_events_filtered_by_operation() {
    let root = tempfile::tempdir().expect("vault");
    let forget = begin_forget(
        root.path(),
        "old.md",
        Some("id-1"),
        None,
        vec![],
        "obsolete",
    )
    .expect("forget intent");
    complete_forget(root.path(), forget).expect("forget completion");
    let update = begin_update(
        root.path(),
        "new.md",
        Some("id-2"),
        vec![],
        "before",
        "after",
        "correction",
    )
    .expect("update intent");
    complete_update(root.path(), update).expect("update completion");

    let events = history(root.path(), 10, Some("UPDATE")).expect("history");
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| event["operation"] == "update"));
    assert!(events[0]["occurred_at"].as_str() >= events[1]["occurred_at"].as_str());
    let limited = history(root.path(), 1, None).expect("limited history");
    assert_eq!(limited.len(), 1);
    assert_eq!(
        history(root.path(), 10, Some("archive")).expect("empty"),
        Vec::<serde_json::Value>::new()
    );
}

#[test]
fn forget_writes_a_synced_tombstone_and_structured_audit_without_note_content() {
    let root = tempfile::tempdir().expect("vault");
    let intent = begin_forget(
        root.path(),
        "notes/private.md",
        Some("note-uuid"),
        Some(&sha256("private note body")),
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
        Some(&sha256(original)),
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
