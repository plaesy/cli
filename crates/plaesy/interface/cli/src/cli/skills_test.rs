use std::ffi::OsString;
use std::fs;
use std::path::Path;

use super::*;

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("test path has a parent")).expect("create test dir");
    fs::write(path, body).expect("write test file");
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("read test file")
}

#[test]
fn resolve_prefers_flag_over_env_settings_and_default() {
    let home = tempfile::tempdir().expect("tempdir");
    let settings = home.path().join(".plaesy").join("settings.json");
    write(&settings, r#"{"skills":{"dir":"/from/settings"}}"#);
    let flag = Path::new("/from/flag");

    let got = resolve_plaesy_dir(home.path(), Some(flag), Some(OsString::from("/from/env")));
    assert_eq!(got, PathBuf::from("/from/flag"));
}

#[test]
fn resolve_prefers_env_over_settings() {
    let home = tempfile::tempdir().expect("tempdir");
    write(
        &home.path().join(".plaesy").join("settings.json"),
        r#"{"skills":{"dir":"/from/settings"}}"#,
    );

    let got = resolve_plaesy_dir(home.path(), None, Some(OsString::from("/from/env")));
    assert_eq!(got, PathBuf::from("/from/env"));
}

#[test]
fn resolve_uses_settings_when_no_flag_or_env() {
    let home = tempfile::tempdir().expect("tempdir");
    write(
        &home.path().join(".plaesy").join("settings.json"),
        r#"{"skills":{"dir":"/from/settings"}}"#,
    );

    assert_eq!(
        resolve_plaesy_dir(home.path(), None, None),
        PathBuf::from("/from/settings")
    );
}

#[test]
fn resolve_falls_back_to_default_store() {
    let home = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        resolve_plaesy_dir(home.path(), None, None),
        home.path().join(".plaesy").join("skills")
    );
}

#[test]
fn resolve_ignores_empty_env_value() {
    let home = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        resolve_plaesy_dir(home.path(), None, Some(OsString::new())),
        home.path().join(".plaesy").join("skills")
    );
}

#[test]
fn settings_malformed_or_empty_dir_falls_through_to_default() {
    let home = tempfile::tempdir().expect("tempdir");
    let settings = home.path().join(".plaesy").join("settings.json");

    write(&settings, "{ not json");
    assert_eq!(settings_skills_dir(&settings), None);

    write(&settings, r#"{"skills":{"dir":""}}"#);
    assert_eq!(settings_skills_dir(&settings), None);

    write(&settings, r#"{"other":1}"#);
    assert_eq!(settings_skills_dir(&settings), None);

    assert_eq!(
        resolve_plaesy_dir(home.path(), None, None),
        home.path().join(".plaesy").join("skills")
    );
}

#[test]
fn entries_missing_dir_is_empty_and_skips_plain_files() {
    let root = tempfile::tempdir().expect("tempdir");
    assert!(entries(&root.path().join("absent"))
        .expect("missing dir")
        .is_empty());

    write(
        &root.path().join("skills").join("alpha").join("SKILL.md"),
        "a",
    );
    write(
        &root.path().join("skills").join("README.txt"),
        "not a skill",
    );
    let names: Vec<String> = entries(&root.path().join("skills"))
        .expect("entries")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_string()]);
}

#[test]
fn provider_dir_rejects_unknown_provider() {
    let home = tempfile::tempdir().expect("tempdir");
    let plaesy = home.path().join(".plaesy").join("skills");
    assert!(provider_dir(home.path(), &plaesy, "nope").is_err());
    assert_eq!(
        provider_dir(home.path(), &plaesy, "claude").expect("claude"),
        home.path().join(".claude").join("skills")
    );
    assert_eq!(
        provider_dir(home.path(), &plaesy, "plaesy").expect("plaesy"),
        plaesy
    );
}

#[test]
fn list_succeeds_when_no_provider_dirs_exist() {
    let home = tempfile::tempdir().expect("tempdir");
    let plaesy = home.path().join(".plaesy").join("skills");
    list(home.path(), &plaesy).expect("list with no skills");
}

#[test]
fn sync_dry_run_writes_nothing() {
    let home = tempfile::tempdir().expect("tempdir");
    let plaesy = home.path().join(".plaesy").join("skills");
    write(&home.path().join(".claude/skills/alpha/SKILL.md"), "alpha");

    sync(home.path(), &plaesy, "claude", "plaesy", false).expect("dry run");
    assert!(!plaesy.exists(), "dry run must not create the target");
}

#[test]
fn sync_apply_copies_only_missing_and_keeps_existing_target_bytes() {
    let home = tempfile::tempdir().expect("tempdir");
    let plaesy = home.path().join(".plaesy").join("skills");
    write(
        &home.path().join(".claude/skills/alpha/SKILL.md"),
        "source alpha",
    );
    write(&home.path().join(".claude/skills/beta/SKILL.md"), "beta");
    write(
        &home.path().join(".claude/skills/beta/refs/notes.md"),
        "nested",
    );
    write(&plaesy.join("alpha").join("SKILL.md"), "target alpha");

    sync(home.path(), &plaesy, "claude", "plaesy", true).expect("apply");

    assert_eq!(read(&plaesy.join("alpha").join("SKILL.md")), "target alpha");
    assert_eq!(read(&plaesy.join("beta").join("SKILL.md")), "beta");
    assert_eq!(
        read(&plaesy.join("beta").join("refs").join("notes.md")),
        "nested"
    );
}

#[test]
fn sync_rejects_same_source_and_target() {
    let home = tempfile::tempdir().expect("tempdir");
    let plaesy = home.path().join(".plaesy").join("skills");
    assert!(sync(home.path(), &plaesy, "claude", "claude", true).is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn sync_does_not_copy_symlinks() {
    let home = tempfile::tempdir().expect("tempdir");
    let plaesy = home.path().join(".plaesy").join("skills");
    let source = home.path().join(".claude").join("skills");
    write(&source.join("real").join("SKILL.md"), "real");
    let link = source.join("linked");

    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(source.join("real"), &link);
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_dir(source.join("real"), &link);

    if let Err(err) = created {
        // Windows needs Developer Mode or admin for symlinks; the copy rule is then not exercised.
        eprintln!("skipping sync_does_not_copy_symlinks: cannot create symlink: {err}");
        return;
    }

    sync(home.path(), &plaesy, "claude", "plaesy", true).expect("apply");
    assert!(plaesy.join("real").join("SKILL.md").exists());
    assert!(
        !plaesy.join("linked").exists(),
        "symlinks must not be copied"
    );
}
