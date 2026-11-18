//! Unit tests for `tests` split out of `stophook.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use chrono::Duration;

fn write_state(root: &Path, run: &str) {
    fs::create_dir_all(root.join(".plaesy")).unwrap();
    fs::write(
        root.join(".plaesy/state.json"),
        format!(r#"{{"autonomous_loop":{run}}}"#),
    )
    .unwrap();
}

#[test]
fn no_state_file_does_not_block() {
    let dir = tempfile::tempdir().unwrap();
    let mut err = Vec::new();
    assert_eq!(exit_code(dir.path(), &b""[..], &mut err), 0);
}

#[test]
fn a_report_ends_the_turn() {
    let dir = tempfile::tempdir().unwrap();
    write_state(dir.path(), r#"{"status":"running"}"#);
    fs::create_dir_all(dir.path().join(LOOP_REPORT_DIR)).unwrap();
    fs::write(
        dir.path().join(LOOP_REPORT_DIR).join(STOP_REPORT_FILE),
        "done",
    )
    .unwrap();
    let mut err = Vec::new();
    assert_eq!(exit_code(dir.path(), &b""[..], &mut err), 0);
}

#[test]
fn already_continued_turn_is_released() {
    let dir = tempfile::tempdir().unwrap();
    write_state(dir.path(), r#"{"status":"running"}"#);
    let mut err = Vec::new();
    assert_eq!(
        exit_code(dir.path(), &br#"{"stop_hook_active":true}"#[..], &mut err),
        0
    );
}

#[test]
fn stale_heartbeat_is_not_live() {
    let run = LoopRun {
        status: "running".into(),
        driver_pid: std::process::id().into(),
        heartbeat_at: (Utc::now() - Duration::hours(1)).to_rfc3339(),
    };
    assert!(!run.is_live(Utc::now()));
}

#[test]
fn a_live_driver_keeps_the_turn_going() {
    let dir = tempfile::tempdir().unwrap();
    let heartbeat = Utc::now().to_rfc3339();
    write_state(
        dir.path(),
        &format!(
            r#"{{"status":"running","driver_pid":{},"heartbeat_at":"{heartbeat}"}}"#,
            std::process::id()
        ),
    );
    let mut err = Vec::new();
    assert_eq!(exit_code(dir.path(), &b"{}"[..], &mut err), 2);
    assert!(String::from_utf8(err).unwrap().contains("still running"));
}

#[test]
fn pending_background_work_releases_the_stop() {
    let dir = tempfile::tempdir().unwrap();
    let heartbeat = Utc::now().to_rfc3339();
    write_state(
        dir.path(),
        &format!(
            r#"{{"status":"running","driver_pid":{},"heartbeat_at":"{heartbeat}"}}"#,
            std::process::id()
        ),
    );
    let mut err = Vec::new();
    assert_eq!(
        exit_code(
            dir.path(),
            &br#"{"background_tasks":[{"id":1}]}"#[..],
            &mut err
        ),
        0
    );
}
