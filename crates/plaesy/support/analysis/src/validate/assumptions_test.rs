//! Unit tests for `tests` split out of `assumptions.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn finds_tags_with_their_lines_and_respects_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".gitignore"), "ignored/\n").unwrap();
    fs::create_dir_all(dir.path().join("ignored")).unwrap();
    fs::write(dir.path().join("ignored/x.md"), "ASSUMED — skip me\n").unwrap();
    fs::write(
        dir.path().join("notes.md"),
        "intro\nASSUMED — default to UTC\nASSUMED - hyphen form\n",
    )
    .unwrap();

    let report = scan(dir.path(), 0).unwrap();
    assert_eq!(report.files_scanned, 1);
    let texts: Vec<_> = report
        .tags
        .iter()
        .map(|t| (t.line, t.text.as_str()))
        .collect();
    assert_eq!(texts, [(2, "default to UTC"), (3, "hyphen form")]);
    assert!(report.stale.is_empty());
}

#[test]
fn untracked_files_cannot_be_judged_stale() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.md"), "ASSUMED — x\n").unwrap();
    let report = scan(dir.path(), 1).unwrap();
    assert_eq!(report.tags.len(), 1);
    assert!(report.stale.is_empty());
}
