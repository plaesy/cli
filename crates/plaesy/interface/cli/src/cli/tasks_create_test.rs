//! Unit tests for `tests` split out of `tasks_create.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn args() -> Args {
    Args {
        priority: "high".into(),
        category: "feature".into(),
        title: "New capability".into(),
        kind: "task".into(),
        parent: None,
        depends_on: vec![],
        labels: vec![],
        assignee: None,
        milestone: None,
    }
}

#[test]
fn invalid_metadata_is_rejected_before_task_creation() {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks::tasks_dir(root.path());
    let mut invalid = args();
    invalid.kind = "invalid".into();

    assert!(run(&invalid, &tasks, "spec").is_err());
    assert!(
        !tasks.exists(),
        "invalid input must not create the task tree"
    );
    assert!(tasks::list_all_tasks(&tasks).unwrap().is_empty());
}

#[test]
fn valid_create_is_not_blocked_by_unrelated_existing_relationship_errors() {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks::tasks_dir(root.path());
    let existing = args();
    run(&existing, &tasks, "spec").unwrap();
    let existing_name = tasks::list_tasks(&tasks, tasks::Status::Backlog)
        .unwrap()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let existing_dir = tasks.join("backlog").join(existing_name);
    let mut existing_issue = Issue::read(
        &tasks,
        tasks::Status::Backlog,
        existing_dir.file_name().unwrap().to_str().unwrap(),
    )
    .unwrap();
    existing_issue.depends_on.push("missing-target".into());
    issue::write_metadata_unlocked(&existing_dir, &existing_issue).unwrap();

    let mut next = args();
    next.title = "Another valid task".into();
    assert!(run(&next, &tasks, "spec").is_ok());
    assert_eq!(
        tasks::list_tasks(&tasks, tasks::Status::Backlog)
            .unwrap()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn duplicate_name_does_not_remove_or_overwrite_existing_task() {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks::tasks_dir(root.path());
    let original = args();
    run(&original, &tasks, "spec").unwrap();
    let task_name = tasks::list_tasks(&tasks, tasks::Status::Backlog)
        .unwrap()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let task_dir = tasks.join("backlog").join(&task_name);
    let task_md_before = fs::read(task_dir.join(tasks::TASK_FILE)).unwrap();
    let metadata_before = fs::read(task_dir.join("issue.json")).unwrap();

    assert!(run(&original, &tasks, "spec").is_err());
    assert_eq!(
        fs::read(task_dir.join(tasks::TASK_FILE)).unwrap(),
        task_md_before
    );
    assert_eq!(
        fs::read(task_dir.join("issue.json")).unwrap(),
        metadata_before
    );
}
