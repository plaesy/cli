//! Unit tests for `notify.rs`, included via `#[path]`.

use super::*;

#[test]
fn add_then_list_keeps_all_four_kinds() {
    let root = tempfile::tempdir().unwrap();
    for kind in Kind::ALL {
        add(root.path(), kind, "s-1", "needs a look").unwrap();
    }
    let kinds: Vec<Kind> = list(root.path()).unwrap().iter().map(|n| n.kind).collect();
    assert_eq!(kinds, Kind::ALL.to_vec());
}

#[test]
fn kind_names_round_trip() {
    for kind in Kind::ALL {
        assert_eq!(Kind::parse(kind.as_str()), Some(kind));
    }
    assert_eq!(Kind::parse("unknown"), None);
}

#[test]
fn mark_read_keeps_the_record() {
    let root = tempfile::tempdir().unwrap();
    let item = add(root.path(), Kind::Merged, "s-1", "PR merged").unwrap();
    assert_eq!(mark_read(root.path(), Target::One(&item.id)).unwrap(), 1);
    // A second mark changes nothing, and the record is still there.
    assert_eq!(mark_read(root.path(), Target::All).unwrap(), 0);
    let stored = list(root.path()).unwrap();
    assert_eq!(stored.len(), 1);
    assert!(stored[0].read_at.is_some());
}

#[test]
fn remove_one_and_all() {
    let root = tempfile::tempdir().unwrap();
    let first = add(root.path(), Kind::Closed, "s-1", "closed").unwrap();
    add(root.path(), Kind::InputNeeded, "s-2", "question").unwrap();
    assert_eq!(remove(root.path(), Target::One(&first.id)).unwrap(), 1);
    assert_eq!(list(root.path()).unwrap().len(), 1);
    assert_eq!(remove(root.path(), Target::All).unwrap(), 1);
    assert!(list(root.path()).unwrap().is_empty());
}

#[test]
fn unknown_id_is_not_found() {
    let root = tempfile::tempdir().unwrap();
    add(root.path(), Kind::Merged, "s-1", "PR merged").unwrap();
    assert!(matches!(
        mark_read(root.path(), Target::One("missing")),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        remove(root.path(), Target::One("missing")),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn invalid_input_is_refused_before_writing() {
    let root = tempfile::tempdir().unwrap();
    assert!(matches!(
        add(root.path(), Kind::Merged, "  ", "text"),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        add(root.path(), Kind::Merged, "s-1", "two\nlines"),
        Err(Error::Invalid(_))
    ));
    let too_long = "x".repeat(MAX_MESSAGE_CHARS + 1);
    assert!(matches!(
        add(root.path(), Kind::Merged, "s-1", &too_long),
        Err(Error::Invalid(_))
    ));
    assert!(list(root.path()).unwrap().is_empty());
}

#[test]
fn store_survives_reopen_and_reads_missing_file_as_empty() {
    let root = tempfile::tempdir().unwrap();
    assert!(list(root.path()).unwrap().is_empty());
    add(root.path(), Kind::ReadyToMerge, "s-9", "ready").unwrap();
    let text = std::fs::read_to_string(root.path().join(".plaesy").join(FILE)).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert_eq!(list(root.path()).unwrap()[0].kind, Kind::ReadyToMerge);
}
