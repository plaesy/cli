//! Unit tests for `tests` split out of `llm.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn segments_alternate_prose_and_code_at_fences() {
    let segments = split_segments("intro line\n```\ncode\n```\nafter\n");
    let kinds: Vec<bool> = segments.iter().map(|s| s.was_prose).collect();
    assert_eq!(kinds, [true, false, true]);
    assert_eq!(segments[2].index, 2);
}

#[test]
fn apply_rewrites_only_the_matching_prose_segment() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.md");
    fs::write(
        &file,
        "This is a long enough prose line for the queue.\n```\nkeep\n```\n",
    )
    .unwrap();
    let ann = dir.path().join("ann.json");
    fs::write(
        &ann,
        r#"{"file":"notes.md","segments":[{"index":0,"text":"Short rewrite.\n```\n"}]}"#,
    )
    .unwrap();
    let result = apply(dir.path(), None, &file, &ann).unwrap();
    assert_eq!(result.relative, "notes.md");
    let merged = fs::read_to_string(&file).unwrap();
    assert!(merged.starts_with("Short rewrite.\n```\nkeep"));
    assert!(backup_path(&file).exists());
}

#[test]
fn a_queue_for_another_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.md");
    fs::write(&file, "text\n").unwrap();
    let ann = dir.path().join("ann.json");
    fs::write(&ann, r#"{"file":"other.md","segments":[]}"#).unwrap();
    assert!(matches!(
        apply(dir.path(), None, &file, &ann),
        Err(LlmError::WrongFile { .. })
    ));
}
