//! Unit tests for `telemetry.rs`, kept in a separate file and included via `#[path]`.

use super::*;
use std::fs;
use std::time::Duration;

fn write_config(root: &Path, body: &str) {
    fs::create_dir_all(root.join(".plaesy")).unwrap();
    fs::write(config_path(root), body).unwrap();
}

#[test]
fn telemetry_is_off_by_default() {
    let root = tempfile::tempdir().unwrap();
    assert!(!is_enabled(root.path()).unwrap());

    record(root.path(), "tasks", true, Duration::from_millis(10)).unwrap();
    assert!(!log_path(root.path()).exists());
    assert_eq!(summarize(root.path()).unwrap().total_runs(), 0);
}

#[test]
fn config_without_telemetry_section_stays_off() {
    let root = tempfile::tempdir().unwrap();
    write_config(root.path(), r#"{"version": 1}"#);

    assert!(!is_enabled(root.path()).unwrap());
    record(root.path(), "tasks", true, Duration::from_millis(10)).unwrap();
    assert!(!log_path(root.path()).exists());
}

#[test]
fn explicit_false_stays_off() {
    let root = tempfile::tempdir().unwrap();
    write_config(
        root.path(),
        r#"{"version": 1, "telemetry": {"enabled": false}}"#,
    );

    assert!(!is_enabled(root.path()).unwrap());
    record(root.path(), "tasks", true, Duration::from_millis(10)).unwrap();
    assert!(!log_path(root.path()).exists());
}

#[test]
fn opt_in_records_and_summary_totals_are_correct() {
    let root = tempfile::tempdir().unwrap();
    write_config(
        root.path(),
        r#"{"version": 1, "telemetry": {"enabled": true}}"#,
    );
    assert!(is_enabled(root.path()).unwrap());

    record(root.path(), "tasks", true, Duration::from_millis(10)).unwrap();
    record(root.path(), "tasks", false, Duration::from_millis(30)).unwrap();
    record(root.path(), "memory", true, Duration::from_millis(5)).unwrap();

    let summary = summarize(root.path()).unwrap();
    assert_eq!(
        summary.commands.get("tasks"),
        Some(&CommandStats {
            runs: 2,
            errors: 1,
            total_ms: 40,
        })
    );
    assert_eq!(
        summary.commands.get("memory"),
        Some(&CommandStats {
            runs: 1,
            errors: 0,
            total_ms: 5,
        })
    );
    assert_eq!(summary.total_runs(), 3);
    assert_eq!(summary.total_errors(), 1);
    assert_eq!(summary.malformed_lines, 0);
}

#[test]
fn malformed_lines_are_counted_and_skipped() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join(".plaesy")).unwrap();
    fs::write(
        log_path(root.path()),
        "{\"command\":\"tasks\",\"ok\":true,\"duration_ms\":7,\"at_ms\":1}\nnot json\n\n",
    )
    .unwrap();

    let summary = summarize(root.path()).unwrap();
    assert_eq!(summary.total_runs(), 1);
    assert_eq!(summary.malformed_lines, 1);
}

#[test]
fn missing_log_gives_empty_summary() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(summarize(root.path()).unwrap(), Summary::default());
}

#[test]
fn invalid_config_is_a_typed_error_not_a_silent_opt_in() {
    let root = tempfile::tempdir().unwrap();
    write_config(root.path(), r#"{"version": 1, "telemetry": "yes"}"#);

    assert!(is_enabled(root.path()).is_err());
    assert!(record(root.path(), "tasks", true, Duration::from_millis(1)).is_err());
    assert!(!log_path(root.path()).exists());
}
