//! Unit tests for `tests` split out of `state.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn save_keeps_other_keys_and_round_trips_the_section() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    fs::write(
        &path,
        r#"{"project":"demo","autonomous_loop":{"status":"running"}}"#,
    )
    .unwrap();
    let state = State {
        status: "paused".into(),
        current_iteration: 4,
        ..State::default()
    };
    save(&path, &state).unwrap();
    let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(doc["project"], "demo");
    assert_eq!(load(&path).unwrap(), state);
}

#[test]
fn missing_file_is_an_empty_state() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        load(&dir.path().join("nope.json")).unwrap(),
        State::default()
    );
}
