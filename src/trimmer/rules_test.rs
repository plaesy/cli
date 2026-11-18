//! Unit tests for `tests` split out of `rules.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_partial_override_keeps_the_missing_sections_from_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trim.json");
    fs::write(&path, r#"{"version":"9","layer2_filler_patterns":[]}"#).unwrap();
    let rules = load(&path);
    assert_eq!(rules.version, "9");
    assert!(rules.layer2_filler_patterns.is_empty());
    assert_eq!(rules.layer2_degree_thresholds.lite_min_degree, 5);
    assert!(rules.layer1_commands.contains_key("cargo test"));
}

#[test]
fn explicit_zero_thresholds_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trim.json");
    fs::write(
        &path,
        r#"{"layer2_degree_thresholds":{"lite_min_degree":0,"full_min_degree":0}}"#,
    )
    .unwrap();
    let rules = load(&path);
    assert_eq!(rules.layer2_degree_thresholds.lite_min_degree, 0);
}
