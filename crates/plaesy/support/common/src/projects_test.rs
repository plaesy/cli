//! Unit tests for `tests` split out of `projects.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
#[test]
fn registration_requires_existing_plaesy_root() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(normalized(dir.path()), Err(Error::NotProject(_))));
    fs::create_dir(dir.path().join(".plaesy")).unwrap();
    assert_eq!(
        normalized(dir.path()).unwrap(),
        fs::canonicalize(dir.path()).unwrap()
    );
}

#[test]
fn registry_records_can_be_filtered_to_existing_plaesy_roots() {
    let dir = tempfile::tempdir().unwrap();
    let project_path = dir.path().join("project");
    fs::create_dir_all(project_path.join(".plaesy")).unwrap();
    let canonical_path = fs::canonicalize(&project_path).unwrap();
    let project = Project {
        id: "p-test".into(),
        path: canonical_path.clone(),
    };
    let missing = Project {
        id: "p-missing".into(),
        path: dir.path().join("missing"),
    };
    let registry = dir.path().join("projects.json");
    write(&registry, &[project.clone(), missing]).unwrap();

    let valid: Vec<_> = read::<Vec<Project>>(&registry)
        .unwrap()
        .into_iter()
        .filter(|entry| normalized(&entry.path).is_ok())
        .collect();
    assert_eq!(valid, vec![project.clone()]);
    assert_eq!(
        valid.iter().find(|entry| entry.id == "p-test"),
        Some(&project)
    );
}

/// A registry in `dir` holding one real Plaesy project, and that project's root.
fn registered_project(dir: &Path, id: &str) -> (Project, PathBuf) {
    let root = dir.join(format!("root-{id}"));
    fs::create_dir_all(root.join(".plaesy")).unwrap();
    let project = Project {
        id: id.into(),
        path: fs::canonicalize(&root).unwrap(),
    };
    let mut entries: Vec<Project> = read(&dir.join(REGISTRY_FILE)).unwrap();
    entries.push(project.clone());
    write(&dir.join(REGISTRY_FILE), &entries).unwrap();
    (project, root)
}

#[test]
fn settings_round_trip_and_unset_drops_empty_projects() {
    let dir = tempfile::tempdir().unwrap();
    let (project, _root) = registered_project(dir.path(), "p-a");
    let (_, before) = settings_in(dir.path(), "p-a").unwrap();
    assert!(before.is_empty());

    set_setting_in(dir.path(), "p-a", "agent.model", Some("fable")).unwrap();
    set_setting_in(dir.path(), "p-a", "branch", Some("main")).unwrap();
    let (found, settings) = settings_in(dir.path(), "p-a").unwrap();
    assert_eq!(found, project);
    assert_eq!(settings.get("agent.model").map(String::as_str), Some("fable"));
    assert_eq!(settings.len(), 2);

    set_setting_in(dir.path(), "p-a", "agent.model", None).unwrap();
    set_setting_in(dir.path(), "p-a", "branch", None).unwrap();
    let config: BTreeMap<String, Settings> = read(&dir.path().join(CONFIG_FILE)).unwrap();
    assert!(config.is_empty());
}

#[test]
fn settings_resolve_by_canonical_path_as_well_as_id() {
    let dir = tempfile::tempdir().unwrap();
    let (project, root) = registered_project(dir.path(), "p-b");
    set_setting_in(dir.path(), &root.to_string_lossy(), "k", Some("v")).unwrap();
    let (_, settings) = settings_in(dir.path(), "p-b").unwrap();
    assert_eq!(settings.get("k").map(String::as_str), Some("v"));
    assert_eq!(project.id, "p-b");
}

#[test]
fn settings_reject_bad_keys_and_values() {
    let dir = tempfile::tempdir().unwrap();
    let (_project, _root) = registered_project(dir.path(), "p-c");
    let long_key = "k".repeat(MAX_KEY_BYTES + 1);
    let long_value = "v".repeat(MAX_VALUE_BYTES + 1);
    for (key, value) in [
        ("", Some("v")),
        ("has space", Some("v")),
        (long_key.as_str(), Some("v")),
        ("k", Some("line\nbreak")),
        ("k", Some(long_value.as_str())),
    ] {
        assert!(
            matches!(
                set_setting_in(dir.path(), "p-c", key, value),
                Err(Error::InvalidSetting(_))
            ),
            "accepted {key:?} {value:?}"
        );
    }
    let (_, settings) = settings_in(dir.path(), "p-c").unwrap();
    assert!(settings.is_empty());
}

#[test]
fn remove_unregisters_and_drops_settings_but_keeps_files() {
    let dir = tempfile::tempdir().unwrap();
    let (project, root) = registered_project(dir.path(), "p-d");
    let (other, _other_root) = registered_project(dir.path(), "p-e");
    set_setting_in(dir.path(), "p-d", "k", Some("v")).unwrap();
    set_setting_in(dir.path(), "p-e", "k", Some("keep")).unwrap();

    assert_eq!(remove_in(dir.path(), "p-d").unwrap(), project);
    assert!(root.join(".plaesy").is_dir());
    let entries: Vec<Project> = read(&dir.path().join(REGISTRY_FILE)).unwrap();
    assert_eq!(entries, vec![other]);
    assert!(matches!(
        settings_in(dir.path(), "p-d"),
        Err(Error::NotFound(_))
    ));
    let (_, kept) = settings_in(dir.path(), "p-e").unwrap();
    assert_eq!(kept.get("k").map(String::as_str), Some("keep"));
}

#[test]
fn remove_works_for_a_stale_entry_whose_root_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let (_project, root) = registered_project(dir.path(), "p-f");
    fs::remove_dir_all(&root).unwrap();
    remove_in(dir.path(), "p-f").unwrap();
    let entries: Vec<Project> = read(&dir.path().join(REGISTRY_FILE)).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn unknown_selector_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        remove_in(dir.path(), "p-missing"),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        set_setting_in(dir.path(), "p-missing", "k", Some("v")),
        Err(Error::NotFound(_))
    ));
}
