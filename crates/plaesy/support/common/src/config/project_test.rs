use std::fs;

use serde_json::Value;

use super::{read_project_value, read_project_value_or_null};

fn plaesy_dir(repo: &std::path::Path) -> std::path::PathBuf {
    let dir = repo.join(".plaesy");
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn nothing_on_disk_reads_as_null() {
    let repo = tempfile::tempdir().unwrap();
    assert_eq!(read_project_value(repo.path()).unwrap(), Value::Null);
    assert_eq!(read_project_value_or_null(repo.path()), Value::Null);
}

#[test]
fn settings_json_is_read_while_config_toml_is_absent() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(
        plaesy_dir(repo.path()).join("settings.json"),
        r#"{"chat":{"web":true}}"#,
    )
    .unwrap();
    let value = read_project_value(repo.path()).unwrap();
    assert_eq!(value["chat"]["web"], Value::Bool(true));
}

#[test]
fn config_toml_wins_over_settings_json() {
    let repo = tempfile::tempdir().unwrap();
    let dir = plaesy_dir(repo.path());
    fs::write(dir.join("settings.json"), r#"{"chat":{"web":true}}"#).unwrap();
    fs::write(dir.join("config.toml"), "[chat]\nweb = false\n").unwrap();
    let value = read_project_value(repo.path()).unwrap();
    assert_eq!(value["chat"]["web"], Value::Bool(false));
}

#[test]
fn a_broken_toml_file_is_an_error_and_reads_as_null_in_the_lenient_form() {
    let repo = tempfile::tempdir().unwrap();
    fs::write(plaesy_dir(repo.path()).join("config.toml"), "{not toml").unwrap();
    assert!(read_project_value(repo.path()).is_err());
    assert_eq!(read_project_value_or_null(repo.path()), Value::Null);
}
