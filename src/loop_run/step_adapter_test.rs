//! Unit tests for `tests` split out of `step_adapter.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn an_openai_step_without_an_endpoint_is_refused() {
    let cfg = Config {
        step: Some(StepConfig {
            kind: "openai".into(),
            ..StepConfig::default()
        }),
        ..Config::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let err = new_step(&cfg, dir.path(), dir.path(), process_env())
        .err()
        .expect("refused");
    assert!(err.contains("base_url and model"));
}

#[test]
fn no_step_command_fails_the_step_instead_of_passing_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut step = new_step(&Config::default(), dir.path(), dir.path(), process_env()).unwrap();
    assert!(step(1, None).is_err());
}
