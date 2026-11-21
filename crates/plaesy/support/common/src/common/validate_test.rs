//! Unit tests for `tests` split out of `validate.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn quiet() -> Logger {
    Logger::new("test")
}

#[test]
fn file_check_rejects_directories_and_missing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "x").unwrap();
    assert!(file_exists(&file, "", &quiet()).is_ok());
    assert!(file_exists(dir.path(), "", &quiet()).is_err());
    let err = file_exists(&dir.path().join("nope"), "", &quiet()).unwrap_err();
    assert!(
        matches!(err, ValidationError::FileMissing { ref description, .. } if description == "file")
    );
}

#[test]
fn directory_check_rejects_files() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    std::fs::write(&file, "x").unwrap();
    assert!(directory_exists(dir.path(), "", &quiet()).is_ok());
    assert!(directory_exists(&file, "", &quiet()).is_err());
}

#[test]
fn missing_command_names_the_command() {
    let err = command_exists("plaesy-definitely-not-a-command-xyz", "", &quiet()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "required command not found: plaesy-definitely-not-a-command-xyz"
    );
}
