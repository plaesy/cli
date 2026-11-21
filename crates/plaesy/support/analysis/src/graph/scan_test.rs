//! Unit tests for `tests` split out of `scan.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn normalize_keeps_the_root_and_drive_letters() {
    assert_eq!(
        normalize_path("/repo/docs", "../src/a.md"),
        "/repo/src/a.md"
    );
    assert_eq!(normalize_path("/repo", "../../../x"), "/x");
    assert_eq!(normalize_path("C:/repo", "../x"), "C:/x");
    assert_eq!(normalize_path("/repo", "/abs/y"), "/abs/y");
}

#[test]
fn links_resolve_only_to_known_nodes() {
    let known: std::collections::BTreeSet<String> = ["docs/b.md".to_owned()].into();
    assert_eq!(
        resolve_rel_link("/repo", "docs/a.md", "b.md#sec", &known),
        Some("docs/b.md".into())
    );
    assert_eq!(
        resolve_rel_link("/repo", "docs/a.md", "https://x", &known),
        None
    );
    assert_eq!(
        resolve_rel_link("/repo", "docs/a.md", "missing.md", &known),
        None
    );
}
