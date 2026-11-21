//! Unit tests for `tests` split out of `layer2.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn fenced_code_is_left_exactly_as_written() {
    let rules = Rules::default();
    let text = "It is important to note that this works.\n```\nIt is important to note that\n```\n";
    let out = compress_prose(text, "lite", &rules);
    assert!(out.contains("It is important to note that\n```"), "{out}");
    assert!(
        !out.starts_with("It is important"),
        "prose must be compressed: {out}"
    );
}

#[test]
fn ultra_removes_stopwords_and_full_normalises_spaces() {
    let rules = Rules::default();
    assert_eq!(
        apply_level("a  very   big  dog", "full", &rules),
        "a very big dog"
    );
    assert_eq!(apply_level("a very big dog", "ultra", &rules), "a big dog");
}
