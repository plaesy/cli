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

    let valid: Vec<_> = read(&registry)
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
