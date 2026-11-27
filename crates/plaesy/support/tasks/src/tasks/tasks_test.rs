//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    let tasks = tasks_dir(dir.path());
    (dir, tasks)
}

#[test]
fn write_task_files_failure_removes_the_already_written_task_file() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("task");
    fs::create_dir(&dir).unwrap();
    fs::create_dir(dir.join(CONTRACT_FILE)).unwrap();

    assert!(write_task_files(&dir, "task body", "{}\n").is_err());
    assert!(!dir.join(TASK_FILE).exists());
    assert!(dir.join(CONTRACT_FILE).is_dir());
}

#[test]
fn status_round_trips_and_rejects_unknown() {
    for s in STATUSES {
        assert_eq!(s.as_str().parse::<Status>().unwrap(), s);
    }
    assert!(matches!(
        "later".parse::<Status>(),
        Err(TasksError::InvalidStatus(_))
    ));
}

#[cfg(unix)]
#[test]
fn list_tasks_rejects_symlinked_ancestor_of_tasks_root() {
    use std::os::unix::fs::symlink;

    let (root, tasks) = project();
    let external = root.path().join("external");
    let alias = root.path().join("alias");
    fs::create_dir_all(external.join("tasks/backlog")).unwrap();
    fs::write(external.join("tasks/backlog/low_external"), "").unwrap();
    symlink(&external, &alias).unwrap();

    let result = list_tasks(&alias.join("tasks"), Status::Backlog);

    assert!(external.join("tasks/backlog/low_external").exists());
    assert!(result.is_err());
    assert_eq!(list_tasks(&tasks, Status::Backlog).unwrap(), None);
}

#[cfg(unix)]
#[test]
fn list_tasks_rejects_symlinked_tasks_root() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let external = tasks.parent().unwrap().join("external-tasks");
    fs::create_dir_all(external.join("backlog")).unwrap();
    symlink(&external, &tasks).unwrap();

    assert!(list_tasks(&tasks, Status::Backlog).is_err());
}

#[cfg(unix)]
#[test]
fn task_frontmatter_handle_rejects_replaced_status_directory() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let status_dir = tasks.join(Status::Backlog.as_str());
    let task_dir = status_dir.join("low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    fs::write(
        task_dir.join(TASK_FILE),
        r#"---
title: internal
---
"#,
    )
    .unwrap();
    let handle = crate::memory::fs_safety::DirHandle::open_nofollow(&tasks).unwrap();
    let external = tasks.parent().unwrap().join("external-status");
    fs::create_dir_all(external.join("low_linked")).unwrap();
    fs::write(
        external.join("low_linked").join(TASK_FILE),
        r#"---
title: external secret
---
"#,
    )
    .unwrap();

    fs::rename(&status_dir, tasks.join("backlog-original")).unwrap();
    symlink(&external, &status_dir).unwrap();

    assert!(
        read_task_frontmatter_from_handle(&handle, Path::new("backlog/low_linked/task.md"))
            .is_err()
    );
    assert_eq!(
        read_frontmatter(&external.join("low_linked").join(TASK_FILE))["title"],
        "external secret"
    );
}

#[cfg(unix)]
#[test]
fn find_task_rejects_symlinked_task_directory() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let external = tasks.parent().unwrap().join("external-task");
    fs::create_dir_all(&external).unwrap();
    let backlog = tasks.join(Status::Backlog.as_str());
    fs::create_dir_all(&backlog).unwrap();
    symlink(&external, backlog.join("low_linked")).unwrap();

    assert!(find_task(&tasks, "low_linked").is_none());
}

#[cfg(unix)]
#[test]
fn list_all_tasks_rejects_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let task_dir = tasks.join(Status::Backlog.as_str()).join("high_linked");
    fs::create_dir_all(&task_dir).unwrap();
    let external = tasks.parent().unwrap().join("external-task.md");
    fs::write(&external, "---\\ntitle: outside\\n---\\n").unwrap();
    symlink(&external, task_dir.join(TASK_FILE)).unwrap();

    assert!(list_all_tasks(&tasks).is_err());
    assert!(check_integrity(&tasks)
        .unwrap()
        .iter()
        .any(|problem| problem.message == "task.md is a symlink"));
}

#[cfg(unix)]
#[test]
fn list_tasks_rejects_symlinked_status_directory() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let backlog = tasks.join("backlog");
    fs::create_dir_all(&backlog).unwrap();
    let external = tasks.parent().unwrap().join("external-backlog");
    fs::rename(&backlog, &external).unwrap();
    symlink(&external, &backlog).unwrap();

    assert!(list_tasks(&tasks, Status::Backlog).is_err());
}

#[cfg(unix)]
#[test]
fn add_task_does_not_create_through_symlinked_ancestor_of_tasks_root() {
    use std::os::unix::fs::symlink;

    let (root, tasks) = project();
    let external = root.path().join("external");
    let alias = root.path().join("alias");
    fs::create_dir_all(external.join("tasks/backlog")).unwrap();
    symlink(&external, &alias).unwrap();

    let result = add_task(&alias.join("tasks"), "spec", "low", "linked", "Linked");

    assert!(result.is_err());
    assert!(!external.join("tasks/backlog/low_linked").exists());
    assert!(!tasks.join("backlog/low_linked").exists());
}

#[cfg(unix)]
#[test]
fn add_task_does_not_create_through_symlinked_tasks_root() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let external = tasks.parent().unwrap().join("external-tasks");
    fs::create_dir_all(external.join("backlog")).unwrap();
    symlink(&external, &tasks).unwrap();

    let result = add_task(&tasks, "spec", "low", "linked", "Linked");

    assert!(result.is_err());
    assert!(!external.join("backlog/low_linked").exists());
}

#[cfg(unix)]
#[test]
fn add_task_does_not_create_through_symlinked_status_directory() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let backlog = tasks.join("backlog");
    fs::create_dir_all(&backlog).unwrap();
    let external = tasks.parent().unwrap().join("external-backlog");
    fs::rename(&backlog, &external).unwrap();
    symlink(&external, &backlog).unwrap();

    let result = add_task(&tasks, "spec", "low", "linked", "Linked");

    assert!(result.is_err());
    assert!(!external.join("low_linked").exists());
}

#[test]
fn topic_of_rejects_path_traversal_from_frontmatter() {
    let (_dir, tasks) = project();
    let name = add_task(&tasks, "s", "low", "safe", "Safe").unwrap();
    let task_file = tasks.join("backlog").join(&name).join(TASK_FILE);
    fs::write(&task_file, "---\nspec: s\ntopic: ../../outside\n---\n").unwrap();
    assert_eq!(topic_of(&tasks, Status::Backlog, &name), None);
}

#[test]
fn spec_and_plan_doc_keep_invalid_topics_inside_plaesy() {
    let (root, _tasks) = project();
    let outside = root.path().join("outside");

    let spec = spec_doc(root.path(), "../../outside");
    let plan = plan_doc(root.path(), "../outside");

    assert!(spec.starts_with(root.path().join(".plaesy/spec")));
    assert!(plan.starts_with(root.path().join(".plaesy/plan")));
    assert!(!spec.starts_with(&outside));
    assert!(!plan.starts_with(&outside));
    assert!(spec.ends_with("__invalid_topic__/spec.md"));
    assert!(plan.ends_with("__invalid_topic__/plan.md"));
}

#[test]
fn read_frontmatter_does_not_strip_unmatched_quotes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("task.md");
    fs::write(&path, "---\ntitle: \"keep opening quote\n---\n").unwrap();
    assert_eq!(read_frontmatter(&path)["title"], "\"keep opening quote");
}

#[test]
fn read_frontmatter_uses_last_colon_in_value() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("task.md");
    fs::write(&path, "---\ntitle: https://example.test/a:b\n---\n").unwrap();
    assert_eq!(read_frontmatter(&path)["title"], "https://example.test/a:b");
}

#[test]
fn task_frontmatter_values_are_preserved_as_strings() {
    let (_dir, tasks) = project();
    let name = add_task(&tasks, "# prod", "low", "safe", "# note").unwrap();
    let frontmatter = read_frontmatter(&tasks.join("backlog").join(name).join(TASK_FILE));
    assert_eq!(frontmatter["spec"], "# prod");
    assert_eq!(frontmatter["title"], "# note");
}

#[test]
fn task_title_cannot_inject_frontmatter_fields() {
    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "low", "safe", "Title\nspec: injected").unwrap();
    let frontmatter = read_frontmatter(&tasks.join("backlog").join(name).join(TASK_FILE));
    assert_eq!(frontmatter["spec"], "auth");
}

#[test]
fn topic_slug_joins_category_and_title_in_kebab_case() {
    assert_eq!(
        topic_slug("Feature", "Login form!!").unwrap(),
        "feature-login-form"
    );
    assert!(matches!(
        topic_slug("---", "  "),
        Err(TasksError::InvalidSlug(_))
    ));
}

#[test]
fn priority_rank_orders_by_prefix() {
    assert!(priority_rank("critical_x") < priority_rank("low_x"));
    assert_eq!(priority_rank("unknown_x"), PRIORITIES.len());
}

#[cfg(unix)]
#[test]
fn spec_and_topic_helpers_reject_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let (_root, tasks) = project();
    let task_dir = tasks.join("backlog").join("low_linked");
    fs::create_dir_all(&task_dir).unwrap();
    let outside = tasks.parent().unwrap().join("external-task.md");
    fs::write(
        &outside,
        "---\nspec: outside-spec\ntopic: outside-topic\n---\n",
    )
    .unwrap();
    symlink(&outside, task_dir.join(TASK_FILE)).unwrap();

    assert_eq!(spec_of(&tasks, Status::Backlog, "low_linked"), "");
    assert_eq!(topic_of(&tasks, Status::Backlog, "low_linked"), None);
}

#[test]
fn read_frontmatter_parses_top_level_keys_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spec.md");
    fs::write(
        &path,
        "---\r\nspec: \"auth\"\n  nested: no\ntitle: 'Login'\n---\nstatus: ignored\n",
    )
    .unwrap();
    let fm = read_frontmatter(&path);
    assert_eq!(fm["spec"], "auth");
    assert_eq!(fm["title"], "Login");
    assert!(!fm.contains_key("nested"));
    assert!(!fm.contains_key("status"));
}

#[cfg(unix)]
#[test]
fn task_status_save_failure_preserves_original_and_cleans_temporary_file() {
    let temp = tempfile::tempdir().unwrap();
    let task_dir = temp.path().join("task");
    fs::create_dir(&task_dir).unwrap();
    fs::create_dir(task_dir.join(TASK_FILE)).unwrap();
    let handle = crate::memory::fs_safety::DirHandle::open_nofollow(&task_dir).unwrap();

    assert!(write_file_atomically(&handle, TASK_FILE, b"updated").is_err());
    assert!(task_dir.join(TASK_FILE).is_dir());
    let names: Vec<_> = fs::read_dir(&task_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, [std::ffi::OsString::from(TASK_FILE)]);
}

#[test]
fn status_frontmatter_is_prepared_without_mutating_source_bytes() {
    let original = b"---\ntitle: Example\nstatus: todo\n---\nbody\n";
    let updated = with_frontmatter_status(original, Status::Doing);
    assert_eq!(original, b"---\ntitle: Example\nstatus: todo\n---\nbody\n");
    assert_eq!(updated, b"---\ntitle: Example\nstatus: doing\n---\nbody\n");
}

#[cfg(unix)]
#[test]
fn moving_task_does_not_write_through_symlinked_task_directory() {
    use std::os::unix::fs::symlink;

    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "low", "symlink-dir", "Symlink").unwrap();
    let task_dir = tasks.join("backlog").join(&name);
    let external = tasks.parent().unwrap().join("external-task-dir");
    fs::rename(&task_dir, &external).unwrap();
    symlink(&external, &task_dir).unwrap();
    let task_file = external.join(TASK_FILE);
    let original = fs::read(&task_file).unwrap();

    let result = move_task(&tasks, &name, Status::Backlog, Status::Todo);

    assert!(result.is_err());
    assert_eq!(fs::read(&task_file).unwrap(), original);
    assert!(task_dir
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg(unix)]
#[test]
fn moving_task_rejects_symlinked_ancestor_of_tasks_root() {
    use std::os::unix::fs::symlink;

    let (root, _tasks) = project();
    let external = root.path().join("external");
    let alias = root.path().join("alias");
    fs::create_dir_all(external.join("tasks/backlog/low_external")).unwrap();
    fs::write(
        external.join("tasks/backlog/low_external/task.md"),
        "---\nstatus: backlog\n---\n",
    )
    .unwrap();
    symlink(&external, &alias).unwrap();

    let result = move_task(
        &alias.join("tasks"),
        "low_external",
        Status::Backlog,
        Status::Todo,
    );

    assert!(result.is_err());
    assert!(external.join("tasks/backlog/low_external").exists());
    assert!(!external.join("tasks/todo/low_external").exists());
}

#[cfg(unix)]
#[test]
fn moving_task_does_not_follow_symlinked_status_directory() {
    use std::os::unix::fs::symlink;

    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "low", "status-link", "Status link").unwrap();
    let backlog = tasks.join("backlog");
    let external = tasks.parent().unwrap().join("external-backlog");
    fs::rename(&backlog, &external).unwrap();
    symlink(&external, &backlog).unwrap();
    let source_task = external.join(&name);
    let original = fs::read(source_task.join(TASK_FILE)).unwrap();

    let result = move_task(&tasks, &name, Status::Backlog, Status::Todo);

    assert!(result.is_err());
    assert_eq!(fs::read(source_task.join(TASK_FILE)).unwrap(), original);
    assert!(backlog.symlink_metadata().unwrap().file_type().is_symlink());
}

#[cfg(unix)]
#[test]
fn moving_task_does_not_write_through_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "low", "symlink", "Symlink").unwrap();
    move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
    let task_file = tasks.join("todo").join(&name).join(TASK_FILE);
    fs::remove_file(&task_file).unwrap();
    let external = tasks.parent().unwrap().join("external-task.md");
    let sentinel = b"external sentinel\n";
    fs::write(&external, sentinel).unwrap();
    symlink(&external, &task_file).unwrap();

    let result = start_task(&tasks, &name);

    assert!(result.is_err());
    assert_eq!(fs::read(&external).unwrap(), sentinel);
    assert!(task_file
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg(unix)]
#[test]
fn block_task_rejects_symlinked_ancestor_of_tasks_root() {
    use std::os::unix::fs::symlink;

    let (root, _tasks) = project();
    let external = root.path().join("external");
    let alias = root.path().join("alias");
    let task_dir = external.join("tasks/doing/low_external");
    fs::create_dir_all(&task_dir).unwrap();
    let task_file = task_dir.join(TASK_FILE);
    let original = b"---\nstatus: doing\n---\nTask body\n";
    fs::write(&task_file, original).unwrap();
    symlink(&external, &alias).unwrap();

    let result = block_task(&alias.join("tasks"), "low_external", "reason");

    assert!(result.is_err());
    assert_eq!(fs::read(task_file).unwrap(), original);
    assert!(external.join("tasks/doing/low_external").exists());
    assert!(!external.join("tasks/blocked/low_external").exists());
}

#[cfg(unix)]
#[test]
fn block_task_does_not_write_through_symlinked_task_directory() {
    use std::os::unix::fs::symlink;

    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "low", "symlink-dir", "Symlink").unwrap();
    move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
    start_task(&tasks, &name).unwrap();
    let task_dir = tasks.join("doing").join(&name);
    let external = tasks.parent().unwrap().join("external-doing-task");
    fs::rename(&task_dir, &external).unwrap();
    symlink(&external, &task_dir).unwrap();
    let task_file = external.join(TASK_FILE);
    let original = fs::read(&task_file).unwrap();

    let result = block_task(&tasks, &name, "external reason");

    assert!(result.is_err());
    assert_eq!(fs::read(&task_file).unwrap(), original);
    assert!(task_dir
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[cfg(unix)]
#[test]
fn block_task_does_not_write_through_symlinked_task_file() {
    use std::os::unix::fs::symlink;

    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "low", "symlink-block", "Symlink").unwrap();
    move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
    start_task(&tasks, &name).unwrap();
    let task_file = tasks.join("doing").join(&name).join(TASK_FILE);
    fs::remove_file(&task_file).unwrap();
    let external = tasks.parent().unwrap().join("external-block.md");
    let sentinel = b"external sentinel\n";
    fs::write(&external, sentinel).unwrap();
    symlink(&external, &task_file).unwrap();

    let result = block_task(&tasks, &name, "external reason");

    assert!(result.is_err());
    assert_eq!(fs::read(&external).unwrap(), sentinel);
    assert!(task_file
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn lifecycle_commands_reject_path_traversal_names() {
    let (_dir, tasks) = project();
    add_task(&tasks, "auth", "medium", "safe", "Safe").unwrap();
    for result in [
        move_task(&tasks, "../outside", Status::Backlog, Status::Todo),
        block_task(&tasks, "../outside", "reason"),
        unblock_task(&tasks, "../outside"),
    ] {
        assert!(matches!(result, Err(TasksError::InvalidSlug(_))));
    }
    assert!(!tasks.join("../outside/task.md").exists());
}

#[test]
fn new_task_template_guides_goal_scope_and_verifiable_acceptance() {
    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "high", "login", "Login").unwrap();
    let body = fs::read_to_string(tasks.join("backlog").join(name).join(TASK_FILE)).unwrap();
    for section in [
        "## Goal",
        "## Scope",
        "## Non-goals",
        "## Deliverables",
        "## Dependencies",
        "## Acceptance criteria",
    ] {
        assert!(body.contains(section), "missing task section: {section}");
    }
}

#[test]
fn add_then_start_complete_moves_the_folder_and_status() {
    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "high", "login-form", "Login form").unwrap();
    assert_eq!(name, "high_login-form");
    assert_eq!(
        pop_next_task(&tasks, "auth").unwrap().as_deref(),
        Some("high_login-form")
    );
    start_task(&tasks, &name).unwrap();
    let (status, _) = find_task(&tasks, &name).unwrap();
    assert_eq!(status, Status::Doing);
    assert!(
        complete_task(&tasks, &name).is_err(),
        "complete must refuse a doing task"
    );
    move_task(&tasks, &name, Status::Doing, Status::Review).unwrap();
    complete_task(&tasks, &name).unwrap();
    let (status, path) = find_task(&tasks, &name).unwrap();
    assert_eq!(status, Status::Done);
    assert!(read_frontmatter(&path.join(TASK_FILE))["status"] == "done");
}

#[test]
fn block_failure_keeps_original_task_content_in_source_status() {
    let (_dir, tasks) = project();
    let name = add_task(&tasks, "auth", "medium", "rollback", "Rollback").unwrap();
    move_task(&tasks, &name, Status::Backlog, Status::Todo).unwrap();
    start_task(&tasks, &name).unwrap();
    let source = tasks.join("doing").join(&name);
    let original = fs::read(source.join(TASK_FILE)).unwrap();
    fs::create_dir_all(tasks.join("blocked").join(&name)).unwrap();

    assert!(block_task(&tasks, &name, "cannot move").is_err());
    assert_eq!(fs::read(source.join(TASK_FILE)).unwrap(), original);
    assert!(source.exists());
}

#[test]
fn wrong_status_names_where_the_task_is() {
    let (_dir, tasks) = project();
    add_task(&tasks, "auth", "low", "x", "X").unwrap();
    let err = start_task(&tasks, "low_x").unwrap_err();
    assert_eq!(err.to_string(), "low_x is in backlog; start needs todo");
}

#[test]
fn block_appends_reason_and_unblock_returns_to_todo() {
    let (_dir, tasks) = project();
    add_task(&tasks, "auth", "medium", "y", "Y").unwrap();
    move_task(&tasks, "medium_y", Status::Backlog, Status::Todo).unwrap();
    start_task(&tasks, "medium_y").unwrap();
    block_task(&tasks, "medium_y", "waiting on API").unwrap();
    let text = fs::read_to_string(tasks.join("blocked/medium_y/task.md")).unwrap();
    assert!(text.contains("## Blocked Reason\nwaiting on API"));
    unblock_task(&tasks, "medium_y").unwrap();
    assert_eq!(find_task(&tasks, "medium_y").unwrap().0, Status::Todo);
}

#[test]
fn next_task_prefers_higher_priority_and_respects_spec() {
    let (_dir, tasks) = project();
    add_task(&tasks, "auth", "low", "a", "A").unwrap();
    add_task(&tasks, "auth", "critical", "b", "B").unwrap();
    add_task(&tasks, "billing", "critical", "c", "C").unwrap();
    assert_eq!(
        next_backlog_task(&tasks, "auth").unwrap().as_deref(),
        Some("critical_b")
    );
    assert_eq!(
        next_backlog_task(&tasks, "billing").unwrap().as_deref(),
        Some("critical_c")
    );
}

#[test]
fn integrity_flags_status_mismatch_and_duplicates() {
    let (_dir, tasks) = project();
    add_task(&tasks, "auth", "low", "z", "Z").unwrap();
    fs::create_dir_all(tasks.join("todo/low_z")).unwrap();
    let problems = check_integrity(&tasks).unwrap();
    assert!(problems
        .iter()
        .any(|p| p.message == "in both backlog and todo"));
}

#[test]
fn concurrent_add_task_creates_one_folder_without_overwriting() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    for round in 0..25 {
        let root = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(root.path());
        let backlog = tasks.join(Status::Backlog.as_str());
        fs::create_dir_all(&backlog).unwrap();
        let tasks = Arc::new(tasks);
        let barrier = Arc::new(Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let tasks = Arc::clone(&tasks);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    add_task(&tasks, "spec", "high", "feature-same", "Same title")
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            1,
            "round {round}"
        );
        assert_eq!(
            results.iter().filter(|result| result.is_err()).count(),
            7,
            "round {round}"
        );
        let task_dir = backlog.join("high_feature-same");
        assert!(task_dir.join(TASK_FILE).is_file(), "round {round}");
        assert!(task_dir.join(CONTRACT_FILE).is_file(), "round {round}");
    }
}

#[test]
fn invalid_slug_and_duplicate_are_rejected() {
    let (_dir, tasks) = project();
    assert!(matches!(
        add_task(&tasks, "s", "low", "a/b", "t"),
        Err(TasksError::InvalidSlug(_))
    ));
    add_task(&tasks, "s", "low", "dup", "t").unwrap();
    assert!(matches!(
        add_task(&tasks, "s", "low", "dup", "t"),
        Err(TasksError::Duplicate(_))
    ));
}

// Symlink guard scope: only components at and below `.plaesy` are checked. A symlink above the
// project root (macOS `/var` -> `/private/var`, a home directory behind a link) is the user's
// choice, so it must not block the project. A symlink inside the project still must.
#[cfg(unix)]
#[test]
fn symlink_above_project_root_is_accepted_but_inside_project_is_rejected() {
    use std::os::unix::fs::symlink;

    let real = tempfile::tempdir().unwrap();
    let alias = tempfile::tempdir().unwrap();
    let alias_root = alias.path().join("link-to-real");
    symlink(real.path(), &alias_root).unwrap();

    // Project reached through a symlinked prefix: accepted.
    let project = real.path().join("proj");
    fs::create_dir_all(project.join(".plaesy/tasks")).unwrap();
    let via_alias = alias_root.join("proj/.plaesy/tasks");
    assert!(!path_has_symlink_component(&via_alias));

    // A standalone tasks collection has the same boundary behavior as `.plaesy/tasks`.
    fs::create_dir_all(project.join("tasks/doing/regular")).unwrap();
    assert!(!path_has_symlink_component(
        &alias_root.join("proj/tasks/doing/regular")
    ));

    // `.plaesy` itself is a symlink to outside the project: rejected.
    let outside = real.path().join("outside");
    fs::create_dir_all(outside.join("tasks")).unwrap();
    let hostile = real.path().join("hostile");
    fs::create_dir_all(&hostile).unwrap();
    symlink(&outside, hostile.join(".plaesy")).unwrap();
    assert!(path_has_symlink_component(&hostile.join(".plaesy/tasks")));

    // A symlink to the tasks folder inside a real project: rejected.
    let linked = real.path().join("linked");
    fs::create_dir_all(linked.join(".plaesy")).unwrap();
    symlink(&outside, linked.join(".plaesy/tasks")).unwrap();
    assert!(path_has_symlink_component(&linked.join(".plaesy/tasks")));
}
