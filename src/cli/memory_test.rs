//! Unit tests for `tests` split out of `memory.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn explicit_vault_is_selected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let args = Args {
        vault: Some(dir.path().to_path_buf()),
        project: false,
        args: Vec::new(),
    };
    assert_eq!(selected_vault(&args).unwrap(), dir.path());
}

#[test]
fn default_database_is_outside_the_vault() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = default_db_path(dir.path()).expect("database path");
    assert!(!db.starts_with(dir.path()));
    assert!(db.to_string_lossy().contains("plaesy"));
    assert!(db.extension().is_some_and(|ext| ext == "db"));
}

#[test]
fn missing_explicit_vault_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing");
    let args = Args {
        vault: Some(missing.clone()),
        project: false,
        args: Vec::new(),
    };
    let error = selected_vault(&args).unwrap_err().to_string();
    assert!(error.contains("memory vault is not a directory"));
    assert!(error.contains(&missing.display().to_string()));
}
