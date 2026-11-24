use super::*;
use plaesy_domain_identity::users::{Role, UserId};

const KEY: &str = "fake-file-key-0123456789abcdef0123456789";

#[test]
fn users_missing_file_loads_empty_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = load_users(&users_path(dir.path())).unwrap();
    assert!(store.get(&UserId::new("alice")).is_none());
}

#[test]
fn users_save_then_load_round_trip_without_plain_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = users_path(dir.path());
    let mut store = UserStore::new();
    store
        .create(UserId::new("alice"), "Alice", Role::Admin, KEY, "0123456789abcdef")
        .unwrap();
    save_users(&path, &store).unwrap();

    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(!on_disk.contains(KEY), "raw key must not reach disk");

    let loaded = load_users(&path).unwrap();
    let user = loaded
        .authenticate(&UserId::new("alice"), KEY)
        .expect("correct key verifies after reload");
    assert_eq!(user.role(), Role::Admin);
    assert!(loaded
        .authenticate(&UserId::new("alice"), "fake-wrong-key-zzzzzzzzzzzzzzzzzzzzzzzzzz")
        .is_err());
}

#[test]
fn users_save_replaces_existing_file_and_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = users_path(dir.path());
    let mut first = UserStore::new();
    first
        .create(UserId::new("alice"), "Alice", Role::Viewer, KEY, "aa")
        .unwrap();
    save_users(&path, &first).unwrap();

    let second = UserStore::new();
    save_users(&path, &second).unwrap();

    let loaded = load_users(&path).unwrap();
    assert!(loaded.get(&UserId::new("alice")).is_none());
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn users_load_rejects_malformed_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = users_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(load_users(&path), Err(UserFileError::Json(_))));
}
