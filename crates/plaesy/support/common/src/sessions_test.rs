//! Unit tests for `tests` split out of `sessions.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
#[test]
fn report_list_heartbeat_stop() {
    let root = tempfile::tempdir().unwrap();
    let files = vec!["src/main.rs".into()];
    let s = report(
        root.path(),
        "s-1",
        "agent",
        "working",
        Some("T-1"),
        &files,
        600,
    )
    .unwrap();
    assert_eq!(list(root.path(), false).unwrap()[0].0, s);
    assert!(heartbeat(root.path(), "s-1", 600).unwrap().expires_at >= s.expires_at);
    stop(root.path(), "s-1").unwrap();
    assert!(list(root.path(), true).unwrap().is_empty());
}
#[test]
fn expired_session_is_stale_and_hidden_by_default() {
    let root = tempfile::tempdir().unwrap();
    let files = vec![];
    report(root.path(), "s", "a", "working", None, &files, 1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(list(root.path(), false).unwrap().is_empty());
    assert_eq!(list(root.path(), true).unwrap().len(), 1);
    assert!(list(root.path(), true).unwrap()[0].1);
}
#[test]
fn rejects_traversing_file_names() {
    let root = tempfile::tempdir().unwrap();
    assert!(report(
        root.path(),
        "s",
        "a",
        "working",
        None,
        &["../outside".into()],
        60
    )
    .is_err());
}
