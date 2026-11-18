//! Unit tests for `tests` split out of `adapter.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn with_scheme_adds_https_only_when_missing() {
    assert_eq!(with_scheme("example.com"), "https://example.com");
    assert_eq!(with_scheme("https://example.com"), "https://example.com");
}

#[test]
fn opt_or_treats_an_empty_value_as_missing() {
    let mut opts = Options::new();
    opts.insert("action".into(), String::new());
    assert_eq!(opt_or(&opts, "action", "search"), "search");
    assert_eq!(opt(&opts, "absent"), "");
}
