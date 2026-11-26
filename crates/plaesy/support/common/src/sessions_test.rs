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
fn re_report_keeps_first_seen_time() {
    let root = tempfile::tempdir().unwrap();
    let first = report(root.path(), "s-keep", "a", "working", None, &[], 600).unwrap();
    let second = report(root.path(), "s-keep", "a", "idle", None, &[], 600).unwrap();
    assert!(first.created_at.is_some());
    assert_eq!(first.created_at, second.created_at);
}
#[test]
fn legacy_record_without_created_at_has_no_age() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".plaesy")).unwrap();
    let legacy = r#"[{"id":"old","agent":"a","state":"working","task":null,"files":[],"updated_at":1,"expires_at":4102444800}]"#;
    std::fs::write(root.path().join(".plaesy").join("sessions.json"), legacy).unwrap();
    let (session, stale) = list(root.path(), false).unwrap().remove(0);
    assert!(!stale);
    assert_eq!(session.created_at, None);
    assert_eq!(age_secs(&session), None);
}
#[test]
fn new_session_reports_a_small_age() {
    let root = tempfile::tempdir().unwrap();
    let s = report(root.path(), "s-age", "a", "working", None, &[], 600).unwrap();
    assert!(age_secs(&s).is_some_and(|age| age < 5));
}
#[test]
fn format_age_uses_the_largest_whole_unit() {
    assert_eq!(format_age(0), "0s");
    assert_eq!(format_age(59), "59s");
    assert_eq!(format_age(60), "1m");
    assert_eq!(format_age(3_599), "59m");
    assert_eq!(format_age(3_600), "1h");
    assert_eq!(format_age(86_400), "1d");
}
#[test]
fn branch_is_none_without_a_git_entry_and_current_branch_with_one() {
    let plain = tempfile::tempdir().unwrap();
    assert_eq!(branch(plain.path()), None);

    let repo = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(args)
            .output()
            .expect("git must be on PATH for this test");
        assert!(out.status.success(), "git {args:?} failed");
    };
    git(&["init", "-q", "-b", "fitur-sesi"]);
    git(&["config", "user.name", "Tes"]);
    git(&["config", "user.email", "tes@example.invalid"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.path().join("a.txt"), "awal").unwrap();
    git(&["add", "--", "a.txt"]);
    git(&["commit", "-q", "-m", "awal"]);
    assert_eq!(branch(repo.path()).as_deref(), Some("fitur-sesi"));
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
