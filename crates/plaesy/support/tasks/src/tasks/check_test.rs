//! Unit tests for `tests` split out of `check.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::tasks::{add_task, tasks_dir};

#[test]
fn backtick_paths_need_a_slash_and_no_placeholders() {
    let refs = extract_references(
        "see `src/lib.rs` and `tasks.rs` and `plaesy tasks check` and `a/<b>`",
        false,
    );
    let paths: Vec<_> = refs.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(paths, ["src/lib.rs"]);
}

#[test]
fn links_skip_schemes_absolute_paths_and_anchors_strip() {
    let refs = extract_references(
        "[a](https://x.y) [b](/abs/p) [c](#top) [d](docs/x.md#sec) [e](../up.md?q=1)",
        false,
    );
    let paths: Vec<_> = refs.iter().map(|r| (r.path.as_str(), r.is_link)).collect();
    assert_eq!(paths, [("docs/x.md", true), ("../up.md", true)]);
}

#[test]
fn new_marker_excludes_planned_files_in_plan_only() {
    let text = "`src/new.rs` (NEW) and `src/old.rs`";
    assert_eq!(extract_references(text, true).len(), 1);
    assert_eq!(extract_references(text, false).len(), 2);
}

#[test]
fn references_must_resolve_inside_project_root() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy/spec/chk/docs")).unwrap();
    std::fs::write(dir.path().join("README.md"), "").unwrap();
    std::fs::write(dir.path().join(".plaesy/spec/chk/docs/local.md"), "").unwrap();
    #[cfg(unix)]
    let outside = tempfile::NamedTempFile::new().unwrap();
    let tasks = tasks_dir(dir.path());
    let name = add_task(&tasks, "s", "low", "check", "Check").unwrap();
    let task_file = tasks
        .join("backlog")
        .join(&name)
        .join(crate::tasks::TASK_FILE);
    std::fs::write(&task_file, "---\nspec: s\ntopic: chk\n---\n").unwrap();
    let spec = super::spec_doc(dir.path(), "chk");
    std::fs::write(
        &spec,
        "[inside](../../../README.md) [outside](../../../../../etc/passwd)",
    )
    .unwrap();
    let plan = super::plan_doc(dir.path(), "chk");
    std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), dir.path().join("src/external-link")).unwrap();
    std::fs::write(&plan, "`src/external-link`\n").unwrap();

    let mut out = Vec::new();
    let err = check_task(&tasks, &name, &mut out).unwrap_err();
    assert!(matches!(err, TasksError::MissingReferences(2)));
    let report = String::from_utf8(out).unwrap();
    assert!(report.contains("missing ../../../../../etc/passwd"));
    assert!(report.contains("missing src/external-link"));
    assert!(!report.contains("missing ../../../README.md"));
}

#[cfg(unix)]
#[test]
fn symlinked_spec_is_rejected_without_reporting_external_contents() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(dir.path());
    let name = add_task(&tasks, "s", "low", "check", "Check").unwrap();
    std::fs::write(
        tasks
            .join("backlog")
            .join(&name)
            .join(crate::tasks::TASK_FILE),
        "---\nspec: s\ntopic: chk\n---\n",
    )
    .unwrap();
    let spec = super::spec_doc(dir.path(), "chk");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    let outside = dir.path().join("external-spec.md");
    std::fs::write(&outside, "`SYNTHETIC_EXTERNAL_REFERENCE/file.rs`\n").unwrap();
    symlink(&outside, &spec).unwrap();

    let mut out = Vec::new();
    let result = check_task(&tasks, &name, &mut out);
    assert!(result.is_err(), "symlinked spec must be rejected");
    assert!(!String::from_utf8(out)
        .unwrap()
        .contains("SYNTHETIC_EXTERNAL_REFERENCE"));
}

#[cfg(unix)]
#[test]
fn symlinked_plan_topic_directory_is_rejected() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks = tasks_dir(dir.path());
    let name = add_task(&tasks, "s", "low", "check", "Check").unwrap();
    std::fs::write(
        tasks
            .join("backlog")
            .join(&name)
            .join(crate::tasks::TASK_FILE),
        "---\nspec: s\ntopic: chk\n---\n",
    )
    .unwrap();
    let spec = super::spec_doc(dir.path(), "chk");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "No references.\n").unwrap();
    let outside = dir.path().join("external-topic");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(
        outside.join("plan.md"),
        "`SYNTHETIC_EXTERNAL_REFERENCE/file.rs`\n",
    )
    .unwrap();
    let plan_parent = dir.path().join(".plaesy/plan");
    std::fs::create_dir_all(&plan_parent).unwrap();
    symlink(&outside, plan_parent.join("chk")).unwrap();

    let mut out = Vec::new();
    assert!(check_task(&tasks, &name, &mut out).is_err());
    assert!(!String::from_utf8(out)
        .unwrap()
        .contains("SYNTHETIC_EXTERNAL_REFERENCE"));
}

#[test]
fn task_without_topic_is_rejected() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    let tasks = tasks_dir(dir.path());
    let name = add_task(&tasks, "s", "low", "bare", "Bare").unwrap();
    std::fs::write(
        tasks
            .join("backlog")
            .join(&name)
            .join(crate::tasks::TASK_FILE),
        "---\nspec: s\ntopic:\n---\n",
    )
    .unwrap();
    let mut out = Vec::new();
    let err = check_task(&tasks, &name, &mut out).unwrap_err();
    assert!(matches!(err, TasksError::NoTopic(_)));
}

#[test]
fn check_reports_missing_and_passes_when_present() {
    let dir = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/real.rs"), "").unwrap();
    let tasks = tasks_dir(dir.path());
    let name = add_task(&tasks, "s", "low", "chk", "Chk").unwrap();
    let task_file = tasks
        .join("backlog")
        .join(&name)
        .join(crate::tasks::TASK_FILE);
    std::fs::write(&task_file, "---\nspec: s\ntopic: chk\n---\n").unwrap();
    let spec = super::spec_doc(dir.path(), "chk");
    std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
    std::fs::write(&spec, "`src/real.rs` and `src/gone.rs`\n").unwrap();

    let mut out = Vec::new();
    let err = check_task(&tasks, &name, &mut out).unwrap_err();
    assert!(matches!(err, TasksError::MissingReferences(1)));
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("spec.md:1: missing src/gone.rs"));

    std::fs::write(&spec, "`src/real.rs`\n").unwrap();
    let mut ok_out = Vec::new();
    check_task(&tasks, &name, &mut ok_out).unwrap();
    assert!(String::from_utf8(ok_out)
        .unwrap()
        .contains("[!] incomplete"));
}
