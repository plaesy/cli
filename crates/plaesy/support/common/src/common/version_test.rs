//! Unit tests for `tests` split out of `version.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn accepts_strict_semver_and_trims_whitespace() {
    assert_eq!(normalize(" 0.0.1\n"), "0.0.1");
    assert_eq!(normalize("10.20.30"), "10.20.30");
}

#[test]
fn rejects_anything_else() {
    for bad in ["", "1.2", "1.2.3.4", "v1.2.3", "1.2.x", "1..3", "1.2.3-rc1"] {
        assert_eq!(normalize(bad), FALLBACK_VERSION, "input {bad:?}");
    }
}
