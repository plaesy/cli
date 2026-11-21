//! Unit tests for `tests` split out of `reach.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn unknown_platforms_are_rejected_with_the_list_hint() {
    let err = parse_platform("nope").expect_err("unknown");
    assert!(err.contains("plaesy reach list"));
}
