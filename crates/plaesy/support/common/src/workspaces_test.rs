//! Unit tests for `workspaces.rs`. The pure group operations run on in-memory lists, so no test
//! touches the per-user data directory.

use super::*;

fn project(id: &str, path: &Path) -> Project {
    Project {
        id: id.into(),
        path: path.to_path_buf(),
    }
}

fn plaesy_root(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir.join(".plaesy")).unwrap();
    fs::canonicalize(dir).unwrap()
}

#[test]
fn group_names_are_limited_to_safe_characters() {
    assert!(validate_name("client-a_2").is_ok());
    assert!(matches!(validate_name(""), Err(Error::InvalidName(_))));
    assert!(matches!(validate_name("a/b"), Err(Error::InvalidName(_))));
    assert!(matches!(validate_name("a b"), Err(Error::InvalidName(_))));
    assert!(matches!(
        validate_name(&"x".repeat(MAX_NAME_LEN + 1)),
        Err(Error::InvalidName(_))
    ));
}

#[test]
fn create_keeps_groups_sorted_and_rejects_duplicates() {
    let mut groups = Vec::new();
    create_in(&mut groups, "zeta").unwrap();
    create_in(&mut groups, "alpha").unwrap();
    let names: Vec<_> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["alpha", "zeta"]);
    assert!(matches!(
        create_in(&mut groups, "alpha"),
        Err(Error::AlreadyExists(_))
    ));
}

#[test]
fn delete_of_unknown_group_is_reported() {
    let mut groups = Vec::new();
    create_in(&mut groups, "one").unwrap();
    delete_in(&mut groups, "one").unwrap();
    assert!(groups.is_empty());
    assert!(matches!(
        delete_in(&mut groups, "one"),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn add_is_idempotent_by_id_or_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = plaesy_root(&dir.path().join("p"));
    let mut groups = Vec::new();
    create_in(&mut groups, "ws").unwrap();

    assert!(add_in(&mut groups, "ws", project("p-1", &root)).unwrap());
    assert!(!add_in(&mut groups, "ws", project("p-1", &root)).unwrap());
    assert!(!add_in(&mut groups, "ws", project("p-other", &root)).unwrap());
    assert_eq!(groups[0].members.len(), 1);
}

#[test]
fn add_to_unknown_group_is_reported() {
    let mut groups = Vec::new();
    assert!(matches!(
        add_in(&mut groups, "missing", project("p-1", Path::new("/x"))),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn remove_matches_id_or_path_and_reports_non_members() {
    let dir = tempfile::tempdir().unwrap();
    let a = plaesy_root(&dir.path().join("a"));
    let b = plaesy_root(&dir.path().join("b"));
    let mut groups = Vec::new();
    create_in(&mut groups, "ws").unwrap();
    add_in(&mut groups, "ws", project("p-a", &a)).unwrap();
    add_in(&mut groups, "ws", project("p-b", &b)).unwrap();

    let removed = remove_in(&mut groups, "ws", "p-a").unwrap();
    assert_eq!(removed.id, "p-a");
    let by_path = remove_in(&mut groups, "ws", b.to_str().unwrap()).unwrap();
    assert_eq!(by_path.id, "p-b");
    match remove_in(&mut groups, "ws", "p-a") {
        Err(Error::NotMember {
            workspace,
            selector,
        }) => {
            assert_eq!(workspace, "ws");
            assert_eq!(selector, "p-a");
        }
        other => panic!("expected NotMember, got {other:?}"),
    }
}

#[test]
fn members_report_roots_that_no_longer_exist_without_removing_them() {
    let dir = tempfile::tempdir().unwrap();
    let live = plaesy_root(&dir.path().join("live"));
    let gone_path = dir.path().join("gone");
    fs::create_dir_all(&gone_path).unwrap();
    let gone = fs::canonicalize(&gone_path).unwrap();
    let mut groups = Vec::new();
    create_in(&mut groups, "ws").unwrap();
    add_in(&mut groups, "ws", project("p-live", &live)).unwrap();
    add_in(&mut groups, "ws", project("p-gone", &gone)).unwrap();
    fs::remove_dir_all(&gone_path).unwrap();

    let members = members_in(&groups, "ws").unwrap();
    let report: Vec<_> = members
        .iter()
        .map(|m| (m.project.id.as_str(), m.available))
        .collect();
    assert_eq!(report, [("p-live", true), ("p-gone", false)]);
    assert_eq!(
        groups[0].members.len(),
        2,
        "missing member must stay listed"
    );
}

#[test]
fn members_of_unknown_group_is_reported() {
    assert!(matches!(members_in(&[], "nope"), Err(Error::NotFound(_))));
}

#[test]
fn store_round_trips_and_missing_file_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    assert!(load(&file).unwrap().is_empty());

    let mut groups = Vec::new();
    create_in(&mut groups, "ws").unwrap();
    add_in(&mut groups, "ws", project("p-1", dir.path())).unwrap();
    save(&file, &groups).unwrap();
    assert_eq!(load(&file).unwrap(), groups);
}
