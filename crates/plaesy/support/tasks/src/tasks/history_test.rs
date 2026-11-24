//! Unit tests for `history` (snapshot, list, restore) split out of `history.rs`.

use super::*;

const TASK: &str = "high_feature-alpha";

const TASK_V1: &str = "---\nstatus: doing\n---\nold body\n";
const TASK_V2: &str = "---\nstatus: doing\n---\nnew body\n";

/// Project with one task in `doing`. `issue` is written as `issue.json` when given.
fn project(task_md: &str, issue: Option<&str>) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_path_buf();
    let dir = tasks_dir(&root).join("doing").join(TASK);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(TASK_FILE), task_md).unwrap();
    if let Some(issue) = issue {
        fs::write(dir.join(ISSUE_FILE), issue).unwrap();
    }
    (temp, root)
}

fn task_path(root: &Path) -> PathBuf {
    tasks_dir(root).join("doing").join(TASK).join(TASK_FILE)
}

fn issue_path(root: &Path) -> PathBuf {
    tasks_dir(root).join("doing").join(TASK).join(ISSUE_FILE)
}

fn issue_json(labels: &[&str]) -> String {
    let labels: Vec<String> = labels.iter().map(|l| format!("\"{l}\"")).collect();
    format!("{{\"id\":\"{TASK}\",\"labels\":[{}]}}\n", labels.join(","))
}

#[test]
fn snapshot_copies_task_md_and_issue_json() {
    let (_temp, root) = project(TASK_V1, Some(&issue_json(&["a"])));

    let meta = snapshot_task(&root, TASK, "start").unwrap().unwrap();

    assert_eq!(meta.version, 1);
    assert_eq!(meta.status, "doing");
    assert_eq!(meta.label, "start");
    assert_eq!(
        meta.files,
        vec![TASK_FILE.to_owned(), ISSUE_FILE.to_owned()]
    );
    let version_dir = root
        .join(".plaesy")
        .join(HISTORY_DIR)
        .join(TASK)
        .join("000001");
    assert_eq!(
        fs::read_to_string(version_dir.join(TASK_FILE)).unwrap(),
        TASK_V1
    );
    assert_eq!(
        fs::read_to_string(version_dir.join(ISSUE_FILE)).unwrap(),
        issue_json(&["a"])
    );
}

#[test]
fn snapshot_without_issue_json_records_only_task_md() {
    let (_temp, root) = project(TASK_V1, None);

    let meta = snapshot_task(&root, TASK, "block").unwrap().unwrap();

    assert_eq!(meta.files, vec![TASK_FILE.to_owned()]);
    assert!(!root
        .join(".plaesy")
        .join(HISTORY_DIR)
        .join(TASK)
        .join("000001")
        .join(ISSUE_FILE)
        .exists());
}

#[test]
fn snapshot_of_unknown_task_returns_none_without_writing() {
    let (_temp, root) = project(TASK_V1, None);

    assert!(snapshot_task(&root, "low_other-missing", "start")
        .unwrap()
        .is_none());
    assert!(!root.join(".plaesy").join(HISTORY_DIR).exists());
}

#[test]
fn snapshot_rejects_invalid_task_name() {
    let (_temp, root) = project(TASK_V1, None);

    assert!(matches!(
        snapshot_task(&root, "../escape", "start"),
        Err(HistoryError::Tasks(TasksError::InvalidSlug(_)))
    ));
}

#[test]
fn snapshots_number_versions_in_order() {
    let (_temp, root) = project(TASK_V1, None);

    let first = snapshot_task(&root, TASK, "start").unwrap().unwrap();
    fs::write(task_path(&root), TASK_V2).unwrap();
    let second = snapshot_task(&root, TASK, "complete").unwrap().unwrap();

    assert_eq!(first.version, 1);
    assert_eq!(second.version, 2);
    let versions = list_versions(&root, TASK).unwrap();
    assert_eq!(
        versions.iter().map(|m| m.version).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(versions[1].label, "complete");
}

#[test]
fn list_is_empty_when_task_has_no_history() {
    let (_temp, root) = project(TASK_V1, None);

    assert!(list_versions(&root, TASK).unwrap().is_empty());
}

#[test]
fn list_ignores_interrupted_version_without_meta() {
    let (_temp, root) = project(TASK_V1, None);
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    // A folder with task.md but no meta.json is an interrupted snapshot.
    let partial = root
        .join(".plaesy")
        .join(HISTORY_DIR)
        .join(TASK)
        .join("000009");
    fs::create_dir_all(&partial).unwrap();
    fs::write(partial.join(TASK_FILE), TASK_V2).unwrap();

    let versions = list_versions(&root, TASK).unwrap();

    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    // The next snapshot still gets a fresh number past the partial folder.
    let next = snapshot_task(&root, TASK, "doing").unwrap().unwrap();
    assert_eq!(next.version, 10);
}

#[test]
fn list_rejects_meta_that_does_not_match_its_folder() {
    let (_temp, root) = project(TASK_V1, None);
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    let meta_path = root
        .join(".plaesy")
        .join(HISTORY_DIR)
        .join(TASK)
        .join("000001")
        .join(META_FILE);
    fs::write(
        &meta_path,
        r#"{"version":7,"status":"doing","label":"x","created_unix":0,"files":["task.md"]}"#,
    )
    .unwrap();

    assert!(matches!(
        list_versions(&root, TASK),
        Err(HistoryError::Corrupt { version: 1, .. })
    ));
}

#[test]
fn restore_rewrites_files_and_keeps_pre_restore_state() {
    let (_temp, root) = project(TASK_V1, Some(&issue_json(&[])));
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    fs::write(task_path(&root), TASK_V2).unwrap();
    fs::write(issue_path(&root), issue_json(&["changed"])).unwrap();

    let restored = restore_version(&root, TASK, 1).unwrap();

    assert_eq!(restored.version, 1);
    assert_eq!(restored.preserved_as, 2);
    assert_eq!(fs::read_to_string(task_path(&root)).unwrap(), TASK_V1);
    assert_eq!(
        fs::read_to_string(issue_path(&root)).unwrap(),
        issue_json(&[])
    );
    // The state from before the restore is itself a version, so the restore can be undone.
    let versions = list_versions(&root, TASK).unwrap();
    assert_eq!(versions.len(), 2);
    let undone = restore_version(&root, TASK, 2).unwrap();
    assert_eq!(undone.preserved_as, 3);
    assert_eq!(fs::read_to_string(task_path(&root)).unwrap(), TASK_V2);
    assert_eq!(
        fs::read_to_string(issue_path(&root)).unwrap(),
        issue_json(&["changed"])
    );
}

#[test]
fn restore_removes_issue_json_that_the_version_did_not_have() {
    let (_temp, root) = project(TASK_V1, None);
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    fs::write(issue_path(&root), issue_json(&["later"])).unwrap();

    restore_version(&root, TASK, 1).unwrap();

    assert!(!issue_path(&root).exists());
    assert_eq!(fs::read_to_string(task_path(&root)).unwrap(), TASK_V1);
}

#[test]
fn restore_keeps_the_status_of_the_folder_the_task_is_in_now() {
    let (_temp, root) = project(TASK_V1, None);
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    let doing = tasks_dir(&root).join("doing").join(TASK);
    let done = tasks_dir(&root).join("done");
    fs::create_dir_all(&done).unwrap();
    fs::rename(&doing, done.join(TASK)).unwrap();

    restore_version(&root, TASK, 1).unwrap();

    let restored = fs::read_to_string(done.join(TASK).join(TASK_FILE)).unwrap();
    assert_eq!(restored, "---\nstatus: done\n---\nold body\n");
}

#[test]
fn restore_unknown_version_changes_nothing() {
    let (_temp, root) = project(TASK_V1, None);
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    fs::write(task_path(&root), TASK_V2).unwrap();

    assert!(matches!(
        restore_version(&root, TASK, 5),
        Err(HistoryError::VersionNotFound { version: 5, .. })
    ));
    assert_eq!(fs::read_to_string(task_path(&root)).unwrap(), TASK_V2);
    assert_eq!(list_versions(&root, TASK).unwrap().len(), 1);
}

#[test]
fn restore_fails_when_the_task_is_gone() {
    let (_temp, root) = project(TASK_V1, None);
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    fs::remove_dir_all(tasks_dir(&root).join("doing").join(TASK)).unwrap();

    assert!(matches!(
        restore_version(&root, TASK, 1),
        Err(HistoryError::TaskNotFound(_))
    ));
}

#[test]
fn restore_rolls_back_when_restored_issue_graph_is_invalid() {
    // The saved issue depends on an issue that does not exist, so validation fails.
    let dangling = format!("{{\"id\":\"{TASK}\",\"depends_on\":[\"ghost_missing-task\"]}}\n");
    let (_temp, root) = project(TASK_V1, Some(&dangling));
    snapshot_task(&root, TASK, "start").unwrap().unwrap();
    fs::write(task_path(&root), TASK_V2).unwrap();
    fs::write(issue_path(&root), issue_json(&["keep"])).unwrap();

    assert!(matches!(
        restore_version(&root, TASK, 1),
        Err(HistoryError::Graph(_))
    ));
    assert_eq!(fs::read_to_string(task_path(&root)).unwrap(), TASK_V2);
    assert_eq!(
        fs::read_to_string(issue_path(&root)).unwrap(),
        issue_json(&["keep"])
    );
}

#[test]
fn with_status_replaces_existing_status_and_keeps_line_endings() {
    assert_eq!(
        with_status("---\r\nstatus: doing\r\n---\r\nbody\r\n", Status::Done),
        "---\r\nstatus: done\r\n---\r\nbody\r\n"
    );
}

#[test]
fn with_status_inserts_missing_status_and_ignores_text_without_frontmatter() {
    assert_eq!(
        with_status("---\ntopic: x\n---\nbody\n", Status::Todo),
        "---\nstatus: todo\ntopic: x\n---\nbody\n"
    );
    assert_eq!(with_status("plain text\n", Status::Done), "plain text\n");
}
