//! Unit tests for `tests` split out of `manager.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use std::sync::mpsc::channel;

use serde::Deserialize;

use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sample {
    name: String,
    count: i64,
}

fn sample(name: &str, count: i64) -> Sample {
    Sample {
        name: name.into(),
        count,
    }
}

#[test]
fn missing_file_yields_the_default_without_error() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = ConfigManager::new(dir.path().join("c.json"), sample("def", 1));
    assert_eq!(*mgr.load().unwrap(), sample("def", 1));
    assert!(!dir.path().join("c.json").exists());
}

#[test]
fn save_then_load_round_trips_and_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("c.json");
    let mgr = ConfigManager::new(&path, sample("def", 0));
    mgr.save(sample("saved", 7)).unwrap();

    let fresh = ConfigManager::new(&path, sample("def", 0));
    assert_eq!(*fresh.load().unwrap(), sample("saved", 7));
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn invalid_json_is_an_error_not_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.json");
    fs::write(&path, "{ nope").unwrap();
    let mgr = ConfigManager::new(&path, sample("def", 0));
    assert!(matches!(mgr.load(), Err(ConfigError::Json(_))));
}

#[test]
fn migrate_moves_contents_and_removes_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old.json");
    let new = dir.path().join("new.json");
    fs::write(&old, r#"{"name":"legacy","count":3}"#).unwrap();

    let mgr = ConfigManager::new(&new, sample("def", 0));
    mgr.migrate_from(&old, |bytes| serde_json::from_slice::<Sample>(bytes))
        .unwrap();

    assert!(!old.exists());
    assert_eq!(*mgr.load().unwrap(), sample("legacy", 3));
}

#[test]
fn migrate_with_no_old_file_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = ConfigManager::new(dir.path().join("new.json"), sample("def", 0));
    mgr.migrate_from(&dir.path().join("absent.json"), |_: &[u8]| {
        Ok::<_, String>(sample("x", 1))
    })
    .unwrap();
    assert!(mgr.get().is_none());
}

#[test]
fn save_notifies_registered_callbacks() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = ConfigManager::new(dir.path().join("c.json"), sample("def", 0));
    let (tx, rx) = channel();
    mgr.watch(move |cfg: &Sample| {
        let _ = tx.send(cfg.clone());
    })
    .unwrap();
    mgr.save(sample("changed", 2)).unwrap();
    let seen = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("callback ran");
    assert_eq!(seen, sample("changed", 2));
    mgr.stop_watch();
}

#[test]
fn external_edit_triggers_a_reload_for_watchers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("c.json");
    let mgr = ConfigManager::new(&path, sample("def", 0));
    let (tx, rx) = channel();
    mgr.watch(move |cfg: &Sample| {
        let _ = tx.send(cfg.clone());
    })
    .unwrap();

    // Give the watcher a moment to attach before the edit.
    thread::sleep(Duration::from_millis(200));
    fs::write(&path, r#"{"name":"edited","count":9}"#).unwrap();

    let seen = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("watcher reported the external edit");
    assert_eq!(seen, sample("edited", 9));
    mgr.stop_watch();
}
