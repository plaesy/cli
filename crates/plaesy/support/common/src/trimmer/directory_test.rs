//! Unit tests for `tests` split out of `directory.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_directory_without_recursion_skips_subfolders() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("top.md"), "x").expect("write");
    fs::create_dir(dir.path().join("sub")).expect("mkdir");
    fs::write(dir.path().join("sub").join("deep.md"), "y").expect("write");
    let mut files = Vec::new();
    for entry in sorted_entries(dir.path()).expect("list") {
        if !entry.file_type().expect("type").is_dir() && is_markdown(&entry.path()) {
            files.push(entry.path());
        }
    }
    assert_eq!(files.len(), 1);
}

#[test]
fn recursion_finds_nested_markdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir(dir.path().join("sub")).expect("mkdir");
    fs::write(dir.path().join("sub").join("deep.MD"), "y").expect("write");
    let mut files = Vec::new();
    walk_markdown(dir.path(), &mut files).expect("walk");
    assert_eq!(files.len(), 1);
}
