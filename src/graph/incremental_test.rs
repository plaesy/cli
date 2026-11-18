//! Unit tests for `tests` split out of `incremental.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn same_strings_ignores_order() {
    let a = vec!["x".to_owned(), "y".to_owned()];
    let b = vec!["y".to_owned(), "x".to_owned()];
    assert!(same_strings(&a, &b));
    assert!(!same_strings(&a, &["x".to_owned()]));
}

#[test]
fn a_stale_version_is_discarded() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(BUILD_CACHE_NAME);
    fs::write(
        &path,
        r#"{"version":0,"files":[],"manifest":"","entries":{}}"#,
    )
    .expect("write");
    assert!(load_build_cache(&path).is_none());
}
