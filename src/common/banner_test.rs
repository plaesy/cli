//! Unit tests for `tests` split out of `banner.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn banner_contains_title_and_subtitle_when_given() {
    let mut buf = Vec::new();
    write_banner(&mut buf, "Reload", "refresh .plaesy").unwrap();
    let text = String::from_utf8(buf).unwrap();
    assert!(text.contains("Reload"));
    assert!(text.contains("refresh .plaesy"));
    assert!(text.ends_with(RESET));
}
