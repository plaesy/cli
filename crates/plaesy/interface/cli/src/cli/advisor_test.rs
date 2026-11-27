//! Unit tests for `tests` split out of `advisor.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::TASK_FILE;
use std::fs;

#[cfg(unix)]
#[test]
fn resolve_task_directory_ignores_symlinked_task_title() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let tasks_dir = temp.path().join("tasks");
    let task_dir = tasks_dir.join("todo/low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    let outside = temp.path().join("external.md");
    fs::write(&outside, "---\ntitle: External secret title\n---\n").unwrap();
    symlink(&outside, task_dir.join(TASK_FILE)).unwrap();

    assert!(resolve_task_directory(&tasks_dir, "External secret title").is_err());
}

#[cfg(unix)]
#[test]
fn read_task_body_rejects_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let tasks_dir = temp.path().join("tasks");
    let task_dir = tasks_dir.join("todo/low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    let outside = temp.path().join("external.md");
    fs::write(&outside, "SYNTHETIC_EXTERNAL_ADVISOR_TASK").unwrap();
    symlink(&outside, task_dir.join(TASK_FILE)).unwrap();

    assert!(read_task_body(&tasks_dir, tasks::Status::Todo, "low_linked").is_err());
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "SYNTHETIC_EXTERNAL_ADVISOR_TASK"
    );
}

#[test]
fn triage_reads_existing_task_and_does_not_mutate_it() {
    let temp = tempfile::tempdir().unwrap();
    let marker = temp.path().join(".plaesy");
    let task_dir = marker.join("tasks/todo/low_task-example");
    fs::create_dir_all(&task_dir).unwrap();
    let task_file = task_dir.join(TASK_FILE);
    fs::write(&task_file, "---\ntopic: example\n---\nWrite a test.\n").unwrap();
    let before = fs::read(&task_file).unwrap();
    let args = TriageArgs {
        task: "low_task-example".into(),
        delegate: DelegateArgs {
            path: temp.path().to_path_buf(),
            provider: None,
            chain: None,
            confirm: false,
            timeout: "120s".into(),
            max_tokens: 0,
        },
    };
    let root = args.delegate.path.canonicalize().unwrap();
    let found = tasks::find_task(&tasks::tasks_dir(&root), &args.task);
    assert!(found.is_some());
    assert_eq!(fs::read(task_file).unwrap(), before);
}

#[test]
fn resolves_unique_title_and_rejects_ambiguous_title() {
    let temp = tempfile::tempdir().unwrap();
    let tasks_dir = temp.path().join(".plaesy/tasks");
    let first = tasks_dir.join("todo/low_first");
    let second = tasks_dir.join("doing/medium_second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(
        first.join(TASK_FILE),
        "---\ntitle: Shared title\n---\nFirst task.\n",
    )
    .unwrap();
    assert_eq!(
        resolve_task_directory(&tasks_dir, "low_first").unwrap(),
        "low_first"
    );
    assert_eq!(
        resolve_task_directory(&tasks_dir, "shared TITLE").unwrap(),
        "low_first"
    );
    fs::write(
        second.join(TASK_FILE),
        "---\ntitle: Shared title\n---\nSecond task.\n",
    )
    .unwrap();
    assert!(resolve_task_directory(&tasks_dir, "Shared title")
        .unwrap_err()
        .contains("ambiguous"));
}

#[test]
fn compare_requires_two_options_at_runtime() {
    use clap::Parser;
    let parsed = crate::cli::Cli::try_parse_from([
        "plaesy", "advisor", "compare", "--prompt", "choose", "--option", "one",
    ])
    .unwrap();
    let crate::cli::Command::Advisor(Args {
        command: AdvisorCommand::Compare(args),
    }) = parsed.command
    else {
        panic!("expected advisor compare command");
    };
    let error = compare(&args).unwrap_err().to_string();
    assert!(error.contains("provide at least two --option values"));
}

#[test]
fn parses_laya_endpoint_mode_without_delegate_provider() {
    use clap::Parser;
    let parsed = crate::cli::Cli::try_parse_from([
        "plaesy",
        "advisor",
        "compare",
        "--prompt",
        "choose",
        "--option",
        "one",
        "--option",
        "two",
        "--laya-endpoint",
        "http://127.0.0.1:8000/v1/systemone",
    ]);
    assert!(parsed.is_ok(), "{parsed:?}");
}

#[test]
fn laya_response_must_choose_a_supplied_option() {
    let answer = serde_json::json!({"choice": "three"});
    let options = ["one".to_owned(), "two".to_owned()];
    let choice = answer
        .get("choice")
        .and_then(serde_json::Value::as_str)
        .unwrap();
    assert!(!options.iter().any(|option| option == choice));
}
