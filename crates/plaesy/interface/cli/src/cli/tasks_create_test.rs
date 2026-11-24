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

/// Creates one backlog task and returns its stable ID.
fn parent_task(tasks: &std::path::Path) -> String {
    run(&args(), tasks, "spec").unwrap();
    Issue::load_collection(tasks)
        .unwrap()
        .into_values()
        .next()
        .unwrap()
        .id
}

fn subtask_args(parent: &str, title: &str) -> SubtaskArgs {
    SubtaskArgs {
        parent: parent.into(),
        priority: "high".into(),
        category: "feature".into(),
        title: title.into(),
    }
}

#[test]
fn create_subtask_records_parent_and_subtask_type() {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks::tasks_dir(root.path());
    let parent = parent_task(&tasks);

    run_subtask(&subtask_args(&parent, "Child work"), &tasks, "spec").unwrap();

    let collection = Issue::load_collection(&tasks).unwrap();
    assert_eq!(collection.len(), 2);
    let child = collection
        .values()
        .find(|issue| issue.id != parent)
        .unwrap();
    assert_eq!(child.kind, "subtask");
    assert_eq!(child.parent.as_deref(), Some(parent.as_str()));
    assert!(Issue::validate_collection(&tasks).unwrap().is_empty());
}

#[test]
fn create_subtask_rejects_missing_parent_without_creating_task() {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks::tasks_dir(root.path());

    let error = run_subtask(&subtask_args("missing-parent", "Orphan"), &tasks, "spec").unwrap_err();

    assert!(error
        .to_string()
        .contains("parent task not found: missing-parent"));
    assert!(tasks::list_all_tasks(&tasks).unwrap().is_empty());
}

#[test]
fn create_rejects_blank_parent_before_touching_the_task_tree() {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks::tasks_dir(root.path());
    let mut blank = args();
    blank.parent = Some("  ".into());

    let error = run(&blank, &tasks, "spec").unwrap_err();

    assert!(error.to_string().contains("parent cannot be empty"));
    assert!(tasks::list_all_tasks(&tasks).unwrap().is_empty());
}

#[test]
fn create_subtask_under_a_task_in_another_status_is_accepted() {
    let root = tempfile::tempdir().unwrap();
    let tasks = tasks::tasks_dir(root.path());
    let parent = parent_task(&tasks);
    tasks::move_task(&tasks, &parent, tasks::Status::Backlog, tasks::Status::Todo).unwrap();

    run_subtask(&subtask_args(&parent, "Late child"), &tasks, "spec").unwrap();

    let collection = Issue::load_collection(&tasks).unwrap();
    let children = collection
        .values()
        .filter(|issue| issue.parent.as_deref() == Some(parent.as_str()))
        .count();
    assert_eq!(children, 1);
}
