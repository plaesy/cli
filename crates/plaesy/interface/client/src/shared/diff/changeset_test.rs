use super::*;
use serde_json::json;

fn set(path: &str, to: serde_json::Value) -> Changeset {
    Changeset::from_diff(vec![Change::Changed {
        path: path.to_owned(),
        from: json!(null),
        to,
    }])
}

#[test]
fn changeset_merges_disjoint_changes() {
    let merged = set("/a", json!(1)).merge(&set("/b", json!(2))).unwrap();
    assert_eq!(merged.changes.len(), 2);
}

#[test]
fn changeset_identical_change_is_not_a_conflict() {
    let merged = set("/a", json!(1)).merge(&set("/a", json!(1))).unwrap();
    assert_eq!(merged.changes.len(), 1);
}

#[test]
fn changeset_same_path_different_value_conflicts() {
    let err = set("/a", json!(1)).merge(&set("/a", json!(2))).unwrap_err();
    assert_eq!(err.path, "/a");
}

#[test]
fn changeset_parent_and_child_paths_conflict() {
    let err = set("/a", json!(1))
        .merge(&set("/a/b", json!(2)))
        .unwrap_err();
    assert_eq!(err.path, "/a/b");
    assert!(
        set("/ab", json!(1)).merge(&set("/a", json!(2))).is_ok(),
        "/ab bukan anak dari /a"
    );
}
