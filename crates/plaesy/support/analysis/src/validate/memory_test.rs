//! Unit tests for `tests` split out of `memory.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn flags_external_paths_and_passes_self_contained_notes() {
    let dir = tempfile::tempdir().unwrap();
    let memory = dir.path().join(".plaesy/memory");
    fs::create_dir_all(memory.join("sub")).unwrap();
    fs::write(memory.join("bad.md"), "ok\nsee ~/.claude/plans/x\n").unwrap();
    fs::write(memory.join("sub/good.md"), "see docs/x.md\n").unwrap();

    let report = validate(dir.path()).unwrap();
    assert_eq!(report.files_checked, 2);
    assert_eq!(report.issues.len(), 1);
    let issue = &report.issues[0];
    assert!(issue.file.ends_with("bad.md"));
    let patterns: Vec<_> = issue.matches.iter().map(|(p, _)| *p).collect();
    assert_eq!(patterns, ["~/.claude", "~/.claude/plans"]);
}

#[test]
fn missing_memory_folder_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(validate(dir.path()).is_err());
}
