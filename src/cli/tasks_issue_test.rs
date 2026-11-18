//! Unit tests for `update_tests` split out of `tasks_issue.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::{
    issue::{self, Issue},
    tasks_dir, TASK_FILE,
};
use std::fs;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks_dir(root.path());
    for status in crate::tasks::STATUSES {
        fs::create_dir_all(tasks.join(status.as_str())).unwrap();
    }
    for id in ["a", "b"] {
        let folder = tasks.join("todo").join(id);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(TASK_FILE), "---\ntitle: test\n---\n").unwrap();
        issue::write_metadata_unlocked(
            &folder,
            &Issue {
                id: id.into(),
                status: Status::Todo,
                kind: "task".into(),
                parent: None,
                depends_on: vec![],
                labels: vec!["keep".into()],
                assignee: Some("owner".into()),
                milestone: Some("M1".into()),
            },
        )
        .unwrap();
    }
    (root, tasks)
}

fn args(id: &str) -> UpdateArgs {
    UpdateArgs {
        issue: id.into(),
        kind: None,
        parent: None,
        clear_parent: false,
        deps: vec![],
        clear_deps: false,
        labels: vec![],
        clear_labels: false,
        assignee: None,
        clear_assignee: false,
        milestone: None,
        clear_milestone: false,
    }
}

#[test]
fn concurrent_updates_preserve_both_fields() {
    use std::thread;

    let (_root, tasks) = fixture();
    let tasks = std::sync::Arc::new(tasks);
    let mut labels = args("a");
    labels.labels = vec!["concurrent".into()];
    let mut assignee = args("a");
    assignee.assignee = Some("new-owner".into());
    let tasks_a = std::sync::Arc::clone(&tasks);
    let tasks_b = std::sync::Arc::clone(&tasks);
    let first = thread::spawn(move || update(&labels, &tasks_a).unwrap());
    let second = thread::spawn(move || update(&assignee, &tasks_b).unwrap());
    first.join().unwrap();
    second.join().unwrap();

    let issue = Issue::load_collection(&tasks).unwrap().remove("a").unwrap();
    assert_eq!(issue.labels, vec!["concurrent"]);
    assert_eq!(issue.assignee.as_deref(), Some("new-owner"));
    assert_eq!(issue.milestone.as_deref(), Some("M1"));
}

#[test]
fn update_preserves_unspecified_fields() {
    let (_root, tasks) = fixture();
    let mut update_args = args("a");
    update_args.labels = vec!["new".into()];
    update(&update_args, &tasks).unwrap();
    let issue = Issue::load_collection(&tasks).unwrap().remove("a").unwrap();
    assert_eq!(issue.labels, vec!["new"]);
    assert_eq!(issue.assignee.as_deref(), Some("owner"));
    assert_eq!(issue.milestone.as_deref(), Some("M1"));
}

#[test]
fn update_rejects_blank_values_and_duplicate_labels_without_changing_metadata() {
    let (_root, tasks) = fixture();
    let path = tasks.join("todo/a/issue.json");
    let original = fs::read(&path).unwrap();

    let mut blank_assignee = args("a");
    blank_assignee.assignee = Some("  ".into());
    assert!(update(&blank_assignee, &tasks).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);

    let mut duplicate_labels = args("a");
    duplicate_labels.labels = vec!["dup".into(), "dup".into()];
    assert!(update(&duplicate_labels, &tasks).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn listing_rejects_dangling_relationships_instead_of_reporting_them_as_waiting() {
    let (_root, tasks) = fixture();
    let mut a = Issue::read(&tasks, crate::tasks::Status::Todo, "a").unwrap();
    a.depends_on = vec!["missing".into()];
    issue::write_metadata_unlocked(&tasks.join("todo/a"), &a).unwrap();

    let result = run(
        &Args {
            spec: None,
            kind: None,
            labels: vec![],
            assignee: None,
            milestone: None,
            status: None,
            dependencies: true,
        },
        &tasks,
    );

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("missing dependency missing"));
}

#[test]
fn rejected_parent_cycle_restores_original_json() {
    let (_root, tasks) = fixture();
    let path = tasks.join("todo/a/issue.json");
    let mut b = Issue::read(&tasks, crate::tasks::Status::Todo, "b").unwrap();
    b.parent = Some("a".into());
    issue::write_metadata_unlocked(&tasks.join("todo/b"), &b).unwrap();
    let old = fs::read(&path).unwrap();
    let mut update_args = args("a");
    update_args.parent = Some("b".into());
    assert!(update(&update_args, &tasks).is_err());
    assert_eq!(fs::read(path).unwrap(), old);
}
