//! Unit tests for `tests` split out of `issue.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::tasks_dir;
use chrono::NaiveDate;

#[test]
fn metadata_without_date_fields_still_loads_and_writes_no_date_keys() {
    let legacy = r#"{"id":"a","type":"task","labels":["keep"],"assignee":"owner"}"#;
    let issue: Issue = serde_json::from_str(legacy).unwrap();
    assert_eq!(issue.start_date, None);
    assert_eq!(issue.due_date, None);
    assert_eq!(issue.labels, vec!["keep"]);
    let text = serde_json::to_string(&issue).unwrap();
    assert!(!text.contains("start_date"));
    assert!(!text.contains("due_date"));
}

#[test]
fn duration_accepts_minutes_hours_and_both() {
    assert_eq!(parse_duration_minutes("90m"), Ok(90));
    assert_eq!(parse_duration_minutes("2h"), Ok(120));
    assert_eq!(parse_duration_minutes("1h30m"), Ok(90));
    assert_eq!(format_minutes(90), "1h30m");
    assert_eq!(format_minutes(120), "2h");
    assert_eq!(format_minutes(45), "45m");
    assert_eq!(format_minutes(0), "0m");
}

#[test]
fn duration_rejects_zero_units_order_and_loose_text() {
    for bad in ["", "0m", "0h", "h", "m", "30", "1h30", "30m1h", "1h 30m", "1.5h", "-1h", "+5m"] {
        assert!(parse_duration_minutes(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn metadata_round_trips_iso_dates() {
    let text = r#"{"id":"a","type":"task","start_date":"2026-03-01","due_date":"2026-03-10"}"#;
    let issue: Issue = serde_json::from_str(text).unwrap();
    assert_eq!(issue.start_date, NaiveDate::from_ymd_opt(2026, 3, 1));
    assert_eq!(issue.due_date, NaiveDate::from_ymd_opt(2026, 3, 10));
    let out = serde_json::to_string(&issue).unwrap();
    assert!(out.contains("\"due_date\":\"2026-03-10\""));
}

#[cfg(target_os = "linux")]
#[test]
fn guarded_metadata_write_does_not_follow_task_path_replacement() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(root.path());
    let task = tasks.join("todo/item");
    fs::create_dir_all(&task).unwrap();
    let outside = root.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let sentinel = outside.join("issue.json");
    fs::write(&sentinel, b"external sentinel").unwrap();
    let guard = IssueGraphLock::acquire(&tasks).unwrap();

    fs::rename(&task, tasks.join("todo/item-original")).unwrap();
    symlink(&outside, &task).unwrap();

    let result = guard.write_metadata(&task, &issue("item", None, &[]));

    assert!(result.is_err());
    assert_eq!(fs::read(sentinel).unwrap(), b"external sentinel");
}

#[test]
fn issue_graph_lock_serializes_independent_threads() {
    use std::sync::{mpsc, Arc, Barrier};
    use std::thread;

    let root = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(root.path());
    fs::create_dir_all(&tasks).unwrap();
    let tasks = Arc::new(tasks);
    let entered = Arc::new(Barrier::new(2));
    let (release_tx, release_rx) = mpsc::channel();
    let thread_tasks = Arc::clone(&tasks);
    let thread_entered = Arc::clone(&entered);
    let worker = thread::spawn(move || {
        let _guard = IssueGraphLock::acquire(&thread_tasks).unwrap();
        thread_entered.wait();
        release_rx.recv().unwrap();
    });

    entered.wait();
    assert!(IssueGraphLock::try_acquire(&tasks).unwrap().is_none());
    release_tx.send(()).unwrap();
    worker.join().unwrap();
    assert!(IssueGraphLock::try_acquire(&tasks).unwrap().is_some());
}

#[test]
fn graph_lock_rejects_metadata_write_to_another_collection() {
    let first = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let second =
        tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let first_tasks = tasks_dir(first.path());
    let second_tasks = tasks_dir(second.path());
    let foreign_task = second_tasks.join("todo/foreign");
    fs::create_dir_all(&foreign_task).unwrap();
    let guard = IssueGraphLock::acquire(&first_tasks).unwrap();

    let result = guard.write_metadata(&foreign_task, &issue("foreign", None, &[]));

    assert!(result.is_err());
    assert!(!foreign_task.join("issue.json").exists());
}

#[test]
fn validate_label_accepts_lowercase_tokens_up_to_the_limit() {
    for label in ["bug", "ui-polish", "v2_api", "a", "0"] {
        assert_eq!(validate_label(label), Ok(()), "{label:?}");
    }
    let at_limit = "a".repeat(MAX_LABEL_CHARS);
    assert_eq!(validate_label(&at_limit), Ok(()));
}

#[test]
fn validate_label_rejects_empty_long_spaced_and_uppercase_labels() {
    assert_eq!(validate_label(""), Err(LabelError::Empty));

    let too_long = "a".repeat(MAX_LABEL_CHARS + 1);
    assert_eq!(
        validate_label(&too_long),
        Err(LabelError::TooLong(too_long.clone()))
    );

    for label in [
        "Bug",
        "needs review",
        " bug",
        "bug ",
        "tab\tx",
        "ui/polish",
        "héllo",
    ] {
        assert_eq!(
            validate_label(label),
            Err(LabelError::InvalidCharacter(label.to_owned())),
            "{label:?}"
        );
    }
}

#[test]
fn add_and_remove_label_are_idempotent_and_keep_order() {
    let mut item = issue("item", None, &[]);
    assert_eq!(item.add_label("bug"), Ok(true));
    assert_eq!(item.add_label("ui"), Ok(true));
    assert_eq!(item.add_label("bug"), Ok(false));
    assert_eq!(item.labels, vec!["bug", "ui"]);

    assert_eq!(item.remove_label("bug"), Ok(true));
    assert_eq!(item.remove_label("bug"), Ok(false));
    assert_eq!(item.labels, vec!["ui"]);
}

#[test]
fn add_and_remove_reject_invalid_labels_without_changing_the_issue() {
    let mut item = issue("item", None, &[]);
    item.labels = vec!["keep".into()];
    assert_eq!(
        item.add_label("Bad Label"),
        Err(LabelError::InvalidCharacter("Bad Label".into()))
    );
    assert_eq!(item.remove_label(""), Err(LabelError::Empty));
    assert_eq!(item.labels, vec!["keep"]);
}

#[test]
fn has_labels_requires_every_requested_label() {
    let mut item = issue("item", None, &[]);
    item.labels = vec!["bug".into(), "ui".into()];
    let bug = "bug".to_owned();
    let ui = "ui".to_owned();
    let api = "api".to_owned();
    assert!(item.has_labels(&[]));
    assert!(item.has_labels(std::slice::from_ref(&bug)));
    assert!(item.has_labels(&[bug.clone(), ui]));
    assert!(!item.has_labels(&[bug, api]));
}

#[test]
fn collection_validation_reports_labels_that_break_the_rule() {
    let root = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(root.path());
    for status in crate::tasks::STATUSES {
        fs::create_dir_all(tasks.join(status.as_str())).unwrap();
    }
    let folder = tasks.join("todo/item");
    fs::create_dir_all(&folder).unwrap();
    let mut item = issue("item", None, &[]);
    item.labels = vec!["Bad".into(), "ok".into()];
    write_metadata_unlocked(&folder, &item).unwrap();

    let problems = Issue::validate_collection(&tasks).unwrap();
    assert_eq!(
        problems,
        vec![format!(
            "item: invalid label: {}",
            LabelError::InvalidCharacter("Bad".into())
        )]
    );
}

fn issue(id: &str, parent: Option<&str>, depends_on: &[&str]) -> Issue {
    Issue {
        id: id.into(),
        status: Status::Todo,
        kind: "task".into(),
        parent: parent.map(str::to_owned),
        depends_on: depends_on.iter().map(|s| (*s).into()).collect(),
        labels: vec![],
        assignee: None,
        milestone: None,
        start_date: None,
        due_date: None,
        estimate_minutes: None,
        time_entries: Vec::new(),
    }
}

#[test]
fn cycle_detection_handles_deep_acyclic_graph_without_recursion() {
    let mut records = BTreeMap::new();
    let count = 50_000;
    for i in 0..count {
        let parent = (i + 1 < count).then(|| format!("node-{:05}", i + 1));
        records.insert(
            format!("node-{i:05}"),
            issue(&format!("node-{i:05}"), parent.as_deref(), &[]),
        );
    }
    assert!(find_cycles(&records, |item| item.parent.iter().cloned().collect()).is_empty());

    let mut cyclic_records = records;
    cyclic_records.get_mut("node-49999").unwrap().parent = Some("node-00000".into());
    let cycle_problems = find_cycles(&cyclic_records, |item| {
        item.parent.iter().cloned().collect()
    });
    assert!(
        !cycle_problems.is_empty(),
        "cycle not found: {}",
        cycle_problems.len()
    );
}

#[test]
fn cycle_detection_handles_shared_children_self_edges_and_disconnected_nodes() {
    let mut records = BTreeMap::new();
    records.insert("a".into(), issue("a", None, &[]));
    records.insert("b".into(), issue("b", None, &[]));
    records.insert("c".into(), issue("c", None, &[]));
    records.insert("d".into(), issue("d", None, &[]));

    records.get_mut("a").unwrap().depends_on = vec!["b".into(), "c".into()];
    records.get_mut("b").unwrap().depends_on = vec!["d".into()];
    records.get_mut("c").unwrap().depends_on = vec!["d".into()];
    assert!(find_cycles(&records, |item| item.depends_on.clone()).is_empty());

    records.get_mut("d").unwrap().depends_on = vec!["d".into()];
    let problems = find_cycles(&records, |item| item.depends_on.clone());
    assert!(problems
        .iter()
        .any(|problem| problem.contains("relationship cycle")));
}

#[test]
fn metadata_write_never_overwrites_a_colliding_temporary_file() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let candidate = dir
        .path()
        .join(format!(".issue.json.tmp-{}-0", std::process::id()));
    fs::write(&candidate, "do not overwrite").unwrap();

    write_metadata_unlocked(dir.path(), &issue("item", None, &[])).unwrap();

    assert_eq!(fs::read_to_string(candidate).unwrap(), "do not overwrite");
    let written: Issue =
        serde_json::from_slice(&fs::read(dir.path().join("issue.json")).unwrap()).unwrap();
    assert_eq!(written.id, "item");
}

#[cfg(target_os = "linux")]
#[test]
fn metadata_write_rejects_symlinked_task_directory() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let outside =
        tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    symlink(outside.path(), root.path().join("task-link")).unwrap();

    let result = write_metadata_unlocked(&root.path().join("task-link"), &issue("item", None, &[]));

    assert!(result.is_err());
    assert!(!outside.path().join("issue.json").exists());
}

#[cfg(unix)]
#[test]
fn metadata_write_replaces_symlink_without_modifying_target() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let target = dir.path().join("outside.json");
    let link = dir.path().join("issue.json");
    fs::write(&target, "outside sentinel").unwrap();
    symlink(&target, &link).unwrap();

    write_metadata_unlocked(dir.path(), &issue("item", None, &[])).unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "outside sentinel");
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_file());
}

#[cfg(unix)]
#[test]
fn metadata_write_preserves_existing_file_permissions() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let path = dir.path().join("issue.json");
    fs::write(&path, "old metadata").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

    write_metadata_unlocked(dir.path(), &issue("item", None, &[])).unwrap();

    assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, 0o640);
}

#[cfg(unix)]
#[test]
fn metadata_snapshot_rejects_symlinked_issue_json() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let task = root.path().join("task");
    fs::create_dir_all(&task).unwrap();
    let outside = root.path().join("outside.json");
    fs::write(&outside, "{\\\"secret\\\":true}").unwrap();
    symlink(&outside, task.join("issue.json")).unwrap();

    let error = read_metadata_bytes(&task).unwrap_err();
    assert!(error.contains("issue.json"));
    assert_eq!(
        fs::read_to_string(outside).unwrap(),
        "{\\\"secret\\\":true}"
    );
}

#[cfg(unix)]
#[test]
fn issue_read_rejects_symlinked_metadata() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let task = root.path().join("todo/task");
    fs::create_dir_all(&task).unwrap();
    let outside = root.path().join("outside.json");
    fs::write(
        &outside,
        serde_json::to_vec(&issue("task", None, &[])).unwrap(),
    )
    .unwrap();
    symlink(&outside, task.join("issue.json")).unwrap();

    let error = Issue::read(root.path(), Status::Todo, "task").unwrap_err();
    assert!(error.contains("opening") && error.contains("issue.json"));
}

#[test]
fn reads_legacy_tasks_with_defaults() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let task = dir.path().join("todo").join("high_legacy");
    fs::create_dir_all(&task).unwrap();
    fs::write(task.join(TASK_FILE), "---\ntitle: Legacy\n---\n").unwrap();
    let parsed = Issue::read(dir.path(), Status::Todo, "high_legacy").unwrap();
    assert_eq!(parsed.kind, "task");
    assert_eq!(parsed.id, "high_legacy");
}

#[test]
fn task_move_preserves_issue_id_and_relationships_without_cascade() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(dir.path());
    for status in STATUSES {
        fs::create_dir_all(tasks.join(status.as_str())).unwrap();
    }
    for (id, parent) in [("parent", None), ("child", Some("parent"))] {
        let folder = tasks.join("todo").join(id);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(TASK_FILE), "---\ntitle: Task\n---\n").unwrap();
        write_metadata_unlocked(&folder, &issue(id, parent, &[])).unwrap();
    }

    super::super::move_task(&tasks, "parent", Status::Todo, Status::Doing).unwrap();
    let collection = Issue::load_collection(&tasks).unwrap();
    assert_eq!(collection["parent"].id, "parent");
    assert_eq!(collection["child"].parent.as_deref(), Some("parent"));
    assert!(Issue::validate_collection(&tasks).unwrap().is_empty());
}

#[test]
fn duplicate_folder_names_across_statuses_report_both_statuses() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(dir.path());
    for status in STATUSES {
        fs::create_dir_all(tasks.join(status.as_str()).join("duplicate")).unwrap();
        fs::write(
            tasks
                .join(status.as_str())
                .join("duplicate")
                .join(TASK_FILE),
            "---\ntitle: Duplicate\n---\n",
        )
        .unwrap();
    }
    let error = Issue::load_collection(&tasks).unwrap_err();
    assert!(error.contains("duplicate issue id \"duplicate\""));
    assert!(error.contains("backlog") && error.contains("todo"));
}

#[cfg(unix)]
#[test]
fn issue_read_rejects_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let task = dir.path().join(Status::Todo.as_str()).join("task");
    fs::create_dir_all(&task).unwrap();
    let outside = dir.path().join("outside-task.md");
    fs::write(&outside, "---\ntype: bug\ntitle: external\n---\n").unwrap();
    symlink(outside, task.join(TASK_FILE)).unwrap();

    let error = Issue::read(dir.path(), Status::Todo, "task").unwrap_err();
    assert!(error.contains("task.md"));
}

#[test]
fn issue_read_rejects_path_traversal_name() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let error = Issue::read(dir.path(), Status::Todo, "../outside").unwrap_err();
    assert!(error.contains("invalid task slug"));
}

#[test]
fn rejects_metadata_id_that_differs_from_folder_name() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let task = dir.path().join("todo").join("canonical");
    fs::create_dir_all(&task).unwrap();
    fs::write(task.join(TASK_FILE), "---\ntitle: Task\n---\n").unwrap();
    write_metadata_unlocked(&task, &issue("forged", None, &[])).unwrap();

    let error = Issue::read(dir.path(), Status::Todo, "canonical").unwrap_err();
    assert!(error.contains("does not match task folder canonical"));
}

#[test]
fn rejects_unknown_types_and_duplicate_metadata() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(dir.path());
    fs::create_dir_all(tasks.join("todo")).unwrap();
    let folder = tasks.join("todo").join("item");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(TASK_FILE), "---\\ntitle: Item\\n---\\n").unwrap();
    let mut record = issue("item", None, &["other", "other"]);
    record.kind = "unknown".into();
    record.labels = vec!["tag".into(), "tag".into()];
    write_metadata_unlocked(&folder, &record).unwrap();
    let problems = Issue::validate_collection(&tasks).unwrap();
    assert!(problems.iter().any(|p| p.contains("unknown type")));
    assert!(problems.iter().any(|p| p.contains("duplicate labels")));
    assert!(problems
        .iter()
        .any(|p| p.contains("duplicate dependencies")));
    assert!(problems
        .iter()
        .any(|p| p.contains("missing dependency other")));
}

#[test]
fn detects_missing_references_and_cycles() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(dir.path());
    for status in STATUSES {
        fs::create_dir_all(tasks.join(status.as_str())).unwrap();
    }
    for (id, issue) in [
        ("a", issue("a", Some("b"), &["missing"])),
        ("b", issue("b", Some("a"), &[])),
    ] {
        let folder = tasks.join("todo").join(id);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(TASK_FILE), "---\ntitle: x\n---\n").unwrap();
        write_metadata_unlocked(&folder, &issue).unwrap();
    }
    let problems = Issue::validate_collection(&tasks).unwrap();
    assert!(problems
        .iter()
        .any(|p| p.contains("missing dependency missing")));
    assert!(problems.iter().any(|p| p.contains("cycle detected")));
    assert!(problems
        .iter()
        .any(|p| p.contains("a: relationship cycle detected")));
}
