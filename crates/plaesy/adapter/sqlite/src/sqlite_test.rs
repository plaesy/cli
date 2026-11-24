use super::*;
use serde_json::json;

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("docs.sqlite")).unwrap();
    (dir, store)
}

#[test]
fn sqlite_put_then_get_returns_the_same_document() {
    let (_dir, store) = store();
    let doc = json!({"ir": "flow-ir/1", "name": "checkout"});
    store.put("flow-ir/1", "checkout", &doc).unwrap();
    assert_eq!(store.get("flow-ir/1", "checkout").unwrap(), Some(doc));
}

#[test]
fn sqlite_put_overwrites_existing_document() {
    let (_dir, store) = store();
    store.put("flow-ir/1", "a", &json!({"v": 1})).unwrap();
    store.put("flow-ir/1", "a", &json!({"v": 2})).unwrap();
    assert_eq!(store.get("flow-ir/1", "a").unwrap(), Some(json!({"v": 2})));
    assert_eq!(store.list("flow-ir/1").unwrap(), vec!["a"]);
}

#[test]
fn sqlite_get_missing_document_is_none() {
    let (_dir, store) = store();
    assert_eq!(store.get("flow-ir/1", "nope").unwrap(), None);
}

#[test]
fn sqlite_list_is_sorted_and_scoped_to_kind() {
    let (_dir, store) = store();
    store.put("flow-ir/1", "b", &json!({})).unwrap();
    store.put("flow-ir/1", "a", &json!({})).unwrap();
    store.put("api-ir/1", "z", &json!({})).unwrap();
    assert_eq!(store.list("flow-ir/1").unwrap(), vec!["a", "b"]);
    assert_eq!(store.list("api-ir/1").unwrap(), vec!["z"]);
}

#[test]
fn sqlite_delete_reports_whether_document_existed() {
    let (_dir, store) = store();
    store.put("flow-ir/1", "a", &json!({})).unwrap();
    assert!(store.delete("flow-ir/1", "a").unwrap());
    assert!(!store.delete("flow-ir/1", "a").unwrap());
    assert_eq!(store.get("flow-ir/1", "a").unwrap(), None);
}

#[test]
fn sqlite_rejects_empty_and_control_character_keys() {
    let (_dir, store) = store();
    assert!(matches!(
        store.put("", "a", &json!({})),
        Err(StoreError::InvalidKey(_))
    ));
    assert!(matches!(
        store.get("flow-ir/1", "bad\nid"),
        Err(StoreError::InvalidKey(_))
    ));
    assert!(matches!(
        store.list(&"k".repeat(257)),
        Err(StoreError::InvalidKey(_))
    ));
}

#[test]
fn sqlite_documents_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("docs.sqlite");
    SqliteStore::open(&path)
        .unwrap()
        .put("flow-ir/1", "a", &json!({"kept": true}))
        .unwrap();
    let reopened = SqliteStore::open(&path).unwrap();
    assert_eq!(
        reopened.get("flow-ir/1", "a").unwrap(),
        Some(json!({"kept": true}))
    );
    assert_eq!(reopened.schema_version().unwrap(), SCHEMA_VERSION);
}

#[test]
fn sqlite_migrates_an_unversioned_database_on_open() {
    // Database yang sudah ada tetapi belum punya versi skema (user_version = 0).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.sqlite");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE unrelated (x INTEGER);")
            .unwrap();
    }
    let store = SqliteStore::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    store.put("flow-ir/1", "a", &json!({})).unwrap();
    assert_eq!(store.list("flow-ir/1").unwrap(), vec!["a"]);
}

#[test]
fn sqlite_refuses_a_newer_schema_without_changing_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.sqlite");
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA user_version = 99;").unwrap();
    }
    let error = SqliteStore::open(&path).unwrap_err();
    assert_eq!(
        error,
        StoreError::UnsupportedSchema {
            found: 99,
            supported: SCHEMA_VERSION
        }
    );
    let conn = Connection::open(&path).unwrap();
    assert_eq!(read_version(&conn).unwrap(), 99);
}
