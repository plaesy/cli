//! Unit tests for `tasks_time`, split out so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::{
    issue::{self, Issue},
    tasks_dir, Status, STATUSES, TASK_FILE,
};
use chrono::NaiveDate;
use std::fs;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks_dir(root.path());
    for status in STATUSES {
        fs::create_dir_all(tasks.join(status.as_str())).unwrap();
    }
    let folder = tasks.join("todo").join("a");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(TASK_FILE), "---\ntitle: test\n---\n").unwrap();
    let issue = Issue {
        id: "a".into(),
        status: Status::Todo,
        kind: "task".into(),
        parent: None,
        depends_on: vec![],
        labels: vec![],
        assignee: None,
        milestone: None,
        start_date: None,
        due_date: None,
        estimate_minutes: None,
        time_entries: vec![],
    };
    issue::write_metadata_unlocked(&folder, &issue).unwrap();
    (root, tasks)
}

fn load_a(tasks: &std::path::Path) -> Issue {
    Issue::load_collection(tasks).unwrap().remove("a").unwrap()
}

#[test]
fn log_appends_entries_in_order_and_trims_notes() {
    let (_root, tasks) = fixture();
    log(
        "a",
        "1h30m",
        Some("2026-03-02"),
        Some("  wrote parser  "),
        &tasks,
    )
    .unwrap();
    log("a", "45m", Some("2026-03-03"), None, &tasks).unwrap();

    let issue = load_a(&tasks);
    assert_eq!(issue.spent_minutes(), 135);
    assert_eq!(issue.time_entries.len(), 2);
    assert_eq!(issue.time_entries[0].note.as_deref(), Some("wrote parser"));
    assert_eq!(issue.time_entries[1].note, None);
    assert_eq!(
        issue.time_entries[1].date,
        NaiveDate::from_ymd_opt(2026, 3, 3).unwrap()
    );
    assert!(show("a", &tasks).is_ok());
}

#[test]
fn log_without_date_records_today() {
    let (_root, tasks) = fixture();
    log("a", "20m", None, None, &tasks).unwrap();
    assert_eq!(
        load_a(&tasks).time_entries[0].date,
        chrono::Local::now().date_naive()
    );
}

#[test]
fn rejected_log_inputs_leave_metadata_unchanged() {
    let (_root, tasks) = fixture();
    let path = tasks.join("todo/a/issue.json");
    let original = fs::read(&path).unwrap();

    assert!(log("a", "soon", None, None, &tasks).is_err());
    assert!(log("a", "0m", None, None, &tasks).is_err());
    assert!(log("a", "1h", Some("03/02/2026"), None, &tasks).is_err());
    let error = log("missing", "1h", None, None, &tasks)
        .unwrap_err()
        .to_string();
    assert!(error.contains("issue not found: missing"));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn effort_line_reports_remaining_overrun_or_nothing() {
    assert_eq!(effort_line(Some(120), 90), "Remaining: 30m");
    assert_eq!(effort_line(Some(120), 120), "Remaining: 0m");
    assert_eq!(effort_line(Some(120), 150), "Over estimate by: 30m");
    assert_eq!(effort_line(None, 90), "");
}

#[test]
fn show_succeeds_with_estimate_and_entries() {
    let (_root, tasks) = fixture();
    let mut issue = load_a(&tasks);
    issue.estimate_minutes = Some(120);
    issue::write_metadata_unlocked(&tasks.join("todo/a"), &issue).unwrap();
    log("a", "90m", Some("2026-03-02"), None, &tasks).unwrap();
    assert!(show("a", &tasks).is_ok());
    assert!(show("missing", &tasks).is_err());
}
