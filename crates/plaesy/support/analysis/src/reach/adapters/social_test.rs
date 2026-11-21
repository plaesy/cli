//! Unit tests for `tests` split out of `social.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn exa_without_a_key_reports_it_instead_of_calling_out() {
    let adapter = ExaSearchAdapter::default();
    let deadline = Deadline::after(std::time::Duration::from_secs(5));
    let result = adapter
        .execute("exa-api", "rust", &Options::new(), deadline)
        .expect("known backend");
    assert!(!result.success);
    assert!(result.error.contains("API key"));
}

#[test]
fn opencli_builds_the_platform_command() {
    let args = opencli("reddit", "search", "rust", "c=1");
    assert_eq!(
        args,
        argv(&["opencli", "reddit", "search", "rust", "--cookie", "c=1"])
    );
}
