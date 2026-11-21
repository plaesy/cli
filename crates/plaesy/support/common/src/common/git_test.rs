//! Unit tests for `tests` split out of `git.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn msys_drive_prefix_maps_to_a_drive_letter() {
    assert_eq!(
        msys_to_windows("/c/Users/me/repo").as_deref(),
        Some("C:\\Users\\me\\repo")
    );
    assert_eq!(msys_to_windows("/D/x").as_deref(), Some("D:\\x"));
}

#[test]
fn non_drive_paths_are_not_rewritten() {
    assert_eq!(msys_to_windows("/tmp/repo"), None);
    assert_eq!(msys_to_windows("C:\\already\\native"), None);
    assert_eq!(msys_to_windows("/c"), None);
}
