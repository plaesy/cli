//! Unit tests for `tests` split out of `tasks_run.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

#[cfg(unix)]
use super::load_topic_spec;
use super::{build_task_prompt, load_claimed_task};
use std::fs;

#[cfg(unix)]
#[test]
fn load_claimed_task_rejects_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let tasks = root.path().join("tasks");
    let task_dir = tasks.join("doing/low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    let outside = root.path().join("external.md");
    fs::write(&outside, "SYNTHETIC_EXTERNAL_TASK").unwrap();
    symlink(&outside, task_dir.join(crate::tasks::TASK_FILE)).unwrap();

    let result = load_claimed_task(&tasks, "low_linked");

    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "SYNTHETIC_EXTERNAL_TASK"
    );
}

#[cfg(unix)]
#[test]
fn load_claimed_task_rejects_symlinked_contracts_file() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let tasks = root.path().join("tasks");
    let task_dir = tasks.join("doing/low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    fs::write(
        task_dir.join(crate::tasks::TASK_FILE),
        "---\ntopic: topic\n---\n",
    )
    .unwrap();
    let outside = root.path().join("external-contracts.json");
    fs::write(&outside, "SYNTHETIC_EXTERNAL_CONTRACTS").unwrap();
    symlink(&outside, task_dir.join(crate::tasks::CONTRACT_FILE)).unwrap();

    let result = load_claimed_task(&tasks, "low_linked");

    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "SYNTHETIC_EXTERNAL_CONTRACTS"
    );
}

#[test]
fn load_claimed_task_reads_regular_doing_task() {
    let root = tempfile::tempdir().unwrap();
    let tasks = root.path().join(".plaesy/tasks");
    let task_dir = tasks.join("doing/low_normal");
    fs::create_dir_all(&task_dir).unwrap();
    fs::write(task_dir.join(crate::tasks::TASK_FILE), "task body").unwrap();

    let (resolved_dir, body, contracts) = load_claimed_task(&tasks, "low_normal").unwrap();

    assert_eq!(resolved_dir, task_dir);
    assert_eq!(body, "task body");
    assert!(contracts.is_empty());
}

#[cfg(unix)]
#[test]
fn load_topic_spec_rejects_symlinked_topic_directory() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let specs = root.path().join(".plaesy/spec");
    fs::create_dir_all(&specs).unwrap();
    let outside = root.path().join("external-topic");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("spec.md"), "SYNTHETIC_EXTERNAL_SPEC").unwrap();
    symlink(&outside, specs.join("topic")).unwrap();

    assert!(load_topic_spec(root.path(), "topic").is_err());
    assert_eq!(
        fs::read_to_string(outside.join("spec.md")).unwrap(),
        "SYNTHETIC_EXTERNAL_SPEC"
    );
}

#[test]
fn task_prompt_includes_task_spec_and_contracts() {
    let prompt = build_task_prompt(
        "high_update-api",
        "Task: update the parser",
        "Spec: keep the API stable",
        "Contracts: unit tests pass",
    );
    assert!(prompt.contains("Task: update the parser"));
    assert!(prompt.contains("Spec: keep the API stable"));
    assert!(prompt.contains("Contracts: unit tests pass"));
}
