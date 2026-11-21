//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn env_root_wins_over_platform_default() {
    let got = root_from(
        Some(PathBuf::from("/custom")),
        Some(PathBuf::from("/platform")),
    );
    assert_eq!(got, Some(PathBuf::from("/custom")));
}

#[test]
fn empty_env_root_falls_back_to_platform() {
    let got = root_from(Some(PathBuf::new()), Some(PathBuf::from("/platform")));
    assert_eq!(got, Some(PathBuf::from("/platform")));
}

#[test]
fn file_lives_under_plaesy_folder() {
    let p = file_in_root(Path::new("/r"));
    assert!(p.ends_with(Path::new("Plaesy").join("settings.json")));
}

#[test]
fn round_trip_and_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = file_in_root(dir.path());
    assert_eq!(load(&path).unwrap(), None);
    let s = Settings {
        notes_dir: Some(PathBuf::from("/notes")),
        rerank: Some(false),
        ..Settings::default()
    };
    save(&path, &s).unwrap();
    assert_eq!(load(&path).unwrap(), Some(s));
}

#[test]
fn unset_fields_are_not_written() {
    let text = serde_json::to_string(&Settings::default()).unwrap();
    assert_eq!(text, format!("{{\"version\":{SETTINGS_VERSION}}}"));
}

#[cfg(unix)]
#[test]
fn symlinked_settings_path_is_rejected_on_load_and_replaced_on_save() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("outside.json");
    std::fs::write(&target, "{\"version\":1}\n").unwrap();
    let path = dir.path().join("settings.json");
    symlink(&target, &path).unwrap();

    assert!(load(&path).is_err());
    save(&path, &Settings::default()).unwrap();
    assert!(!std::fs::symlink_metadata(&path)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "{\"version\":1}\n"
    );
    assert_eq!(load(&path).unwrap(), Some(Settings::default()));
}
