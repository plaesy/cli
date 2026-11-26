//! Unit tests for `tasks_label`, split out so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::{
    issue::{self, Issue, MAX_LABEL_CHARS},
    tasks_dir, Status, STATUSES, TASK_FILE,
};
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
    issue::write_metadata_unlocked(
        &folder,
        &Issue {
            id: "a".into(),
            status: Status::Todo,
            kind: "task".into(),
            parent: None,
            depends_on: vec![],
            labels: vec!["keep".into()],
            assignee: None,
            milestone: None,
            start_date: None,
            due_date: None,
            estimate_minutes: None,
            time_entries: Vec::new(),
        },
    )
    .unwrap();
    (root, tasks)
}

fn labels_of(tasks: &Path, id: &str) -> Vec<String> {
    Issue::load_collection(tasks)
        .unwrap()
        .remove(id)
        .unwrap()
        .labels
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn add_appends_new_labels_and_skips_existing_ones() {
    let (_root, tasks) = fixture();
    apply("a", &strings(&["new", "keep"]), Change::Add, &tasks).unwrap();
    assert_eq!(labels_of(&tasks, "a"), strings(&["keep", "new"]));
}

#[test]
fn remove_deletes_only_the_named_labels() {
    let (_root, tasks) = fixture();
    apply("a", &strings(&["keep", "absent"]), Change::Remove, &tasks).unwrap();
    assert!(labels_of(&tasks, "a").is_empty());
}

#[test]
fn invalid_label_fails_without_changing_metadata() {
    let (_root, tasks) = fixture();
    let path = tasks.join("todo/a/issue.json");
    let original = fs::read(&path).unwrap();

    assert!(apply("a", &strings(&["ok", "Not Ok"]), Change::Add, &tasks).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);

    let too_long = "z".repeat(MAX_LABEL_CHARS + 1);
    assert!(apply("a", &[too_long], Change::Add, &tasks).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn unknown_issue_is_an_error() {
    let (_root, tasks) = fixture();
    assert!(apply("missing", &strings(&["bug"]), Change::Add, &tasks).is_err());
}

#[test]
fn no_op_change_does_not_rewrite_metadata() {
    let (_root, tasks) = fixture();
    let path = tasks.join("todo/a/issue.json");
    let original = fs::read(&path).unwrap();
    apply("a", &strings(&["keep"]), Change::Add, &tasks).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}
