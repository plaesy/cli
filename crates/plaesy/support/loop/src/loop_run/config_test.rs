//! Unit tests for `tests` split out of `config.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn defaults_match_the_documented_caps() {
    let cfg = Config::default();
    assert_eq!(cfg.max_run_iterations, 500);
    assert_eq!(cfg.max_wall_minutes, 240);
    assert_eq!(cfg.thresholds[0].min, 90.0);
}

#[test]
fn unknown_keys_are_rejected() {
    let typo: Result<Config, _> = serde_json::from_str(r#"{"max_run_iteration": 3}"#);
    assert!(typo.is_err());
}

#[test]
fn missing_file_is_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        load(&dir.path().join("loop.json")).unwrap(),
        Config::default()
    );
}
