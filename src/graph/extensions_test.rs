//! Unit tests for `tests` split out of `extensions.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn collected_and_source_are_different_sets() {
    assert!(is_collected("docs/readme.md"));
    assert!(!is_source_ext("docs/readme.md"));
    assert!(is_source_ext("src/Main.RS"));
    assert!(!is_collected("image.png"));
}
