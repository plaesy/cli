//! Unit tests for `tests` split out of `fingerprint.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_fingerprint_without_components_satisfies_no_need() {
    let (base, built) = split_components("3|10|1.0.0");
    assert_eq!(base, "3|10|1.0.0");
    assert!(built.is_empty());
}

#[test]
fn components_round_trip() {
    let fp = with_components("3|10|1.0.0", &["structure", "graph"]);
    let (base, built) = split_components(&fp);
    assert_eq!(base, "3|10|1.0.0");
    assert!(built.contains("graph"));
    assert!(!built.contains("symbols"));
}

#[test]
fn dotenv_counts_as_an_env_extension() {
    assert_eq!(ext_of(".env"), "env");
    assert_eq!(ext_of("a/b.TXT"), "txt");
    assert_eq!(ext_of("Makefile"), "");
}
