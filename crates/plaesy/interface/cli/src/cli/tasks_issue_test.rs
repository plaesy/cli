//! Unit tests for `update_tests` split out of `tasks_issue.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::{
    issue::{self, Issue},
    tasks_dir, TASK_FILE,
};
use std::fs;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
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

/// Writes a task folder under `todo/` whose metadata has the given parent.
fn add_issue(tasks: &std::path::Path, id: &str, parent: Option<&str>) {
    let folder = tasks.join("todo").join(id);
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(TASK_FILE), "---\ntitle: test\n---\n").unwrap();
    issue::write_metadata_unlocked(
        &folder,
        &Issue {
            id: id.into(),
            status: Status::Todo,
            kind: "subtask".into(),
            parent: parent.map(str::to_owned),
            depends_on: vec![],
            labels: vec![],
            assignee: None,
            milestone: None,
        },
    )
    .unwrap();
}

#[test]
fn children_lists_only_direct_children() {
    let (_root, tasks) = fixture();
    let mut b = Issue::read(&tasks, Status::Todo, "b").unwrap();
    b.parent = Some("a".into());
    issue::write_metadata_unlocked(&tasks.join("todo/b"), &b).unwrap();
    add_issue(&tasks, "c", Some("b"));

    let issues = Issue::load_collection(&tasks).unwrap();
    let ids: Vec<&str> = direct_children(&issues, "a")
        .into_iter()
        .map(|issue| issue.id.as_str())
        .collect();
    assert_eq!(ids, vec!["b"]);
    assert!(children(&ChildrenArgs { parent: "b".into() }, &tasks).is_ok());
}

#[test]
fn children_of_a_task_without_subtasks_is_empty_and_ok() {
    let (_root, tasks) = fixture();
    let issues = Issue::load_collection(&tasks).unwrap();
    assert!(direct_children(&issues, "a").is_empty());
    assert!(children(&ChildrenArgs { parent: "a".into() }, &tasks).is_ok());
}

#[test]
fn children_rejects_unknown_parent() {
    let (_root, tasks) = fixture();
    let error = children(
        &ChildrenArgs {
            parent: "ghost".into(),
        },
        &tasks,
    )
    .unwrap_err();
    assert!(error.to_string().contains("issue not found: ghost"));
}

#[test]
fn update_rejects_parent_that_does_not_exist() {
    let (_root, tasks) = fixture();
    let path = tasks.join("todo/a/issue.json");
    let old = fs::read(&path).unwrap();
    let mut update_args = args("a");
    update_args.parent = Some("ghost".into());

    let error = update(&update_args, &tasks).unwrap_err();

    assert!(error.to_string().contains("missing parent ghost"));
    assert_eq!(fs::read(path).unwrap(), old);
}

#[test]
fn parent_cycle_through_three_issues_is_rejected_and_restored() {
    let (_root, tasks) = fixture();
    let mut b = Issue::read(&tasks, Status::Todo, "b").unwrap();
    b.parent = Some("a".into());
    issue::write_metadata_unlocked(&tasks.join("todo/b"), &b).unwrap();
    add_issue(&tasks, "c", Some("b"));
    let path = tasks.join("todo/a/issue.json");
    let old = fs::read(&path).unwrap();
    let mut update_args = args("a");
    // Chain a -> c -> b -> a would close a cycle.
    update_args.parent = Some("c".into());

    let error = update(&update_args, &tasks).unwrap_err();

    assert!(error.to_string().contains("relationship cycle detected"));
    assert_eq!(fs::read(path).unwrap(), old);
}
