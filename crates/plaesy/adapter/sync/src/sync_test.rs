use super::*;
use plaesy_adapter_sqlite::SqliteStore;
use serde_json::json;

fn sqlite() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("replica.sqlite")).unwrap();
    (dir, store)
}

#[test]
fn sync_versions_increase_per_change_from_one() {
    let (_dir, store) = sqlite();
    let source = Source::new(store);
    assert_eq!(source.head().unwrap(), 0);
    assert_eq!(source.put("flow-ir/1", "a", &json!({"v": 1})).unwrap(), 1);
    assert_eq!(source.delete("flow-ir/1", "a").unwrap(), 2);
    assert_eq!(source.head().unwrap(), 2);
}

#[test]
fn sync_changes_since_returns_only_newer_changes_in_order() {
    let (_dir, store) = sqlite();
    let source = Source::new(store);
    source.put("flow-ir/1", "a", &json!(1)).unwrap();
    source.put("flow-ir/1", "b", &json!(2)).unwrap();
    source.put("flow-ir/1", "c", &json!(3)).unwrap();
    let changes = source.changes_since(1).unwrap();
    let ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["b", "c"]);
}

#[test]
fn sync_replica_converges_after_pulling_changes() {
    let (_src_dir, src_store) = sqlite();
    let (_rep_dir, rep_store) = sqlite();
    let source = Source::new(src_store);
    let replica = Replica::new(rep_store);

    source.put("flow-ir/1", "a", &json!({"v": 1})).unwrap();
    source.put("flow-ir/1", "a", &json!({"v": 2})).unwrap();
    source.put("flow-ir/1", "b", &json!({"v": 9})).unwrap();
    source.delete("flow-ir/1", "b").unwrap();

    let applied = replica
        .apply_all(&source.changes_since(0).unwrap())
        .unwrap();
    assert_eq!(applied, 4);
    assert_eq!(
        replica.store().get("flow-ir/1", "a").unwrap(),
        Some(json!({"v": 2}))
    );
    assert_eq!(replica.store().get("flow-ir/1", "b").unwrap(), None);
}

#[test]
fn sync_stale_change_is_dropped_by_last_writer_wins() {
    let (_dir, store) = sqlite();
    let replica = Replica::new(store);
    let newer = Change {
        version: 5,
        kind: "flow-ir/1".to_owned(),
        id: "a".to_owned(),
        op: Op::Put(json!({"v": "baru"})),
    };
    let older = Change {
        version: 3,
        kind: "flow-ir/1".to_owned(),
        id: "a".to_owned(),
        op: Op::Put(json!({"v": "lama"})),
    };
    assert!(replica.apply(&newer).unwrap());
    assert!(
        !replica.apply(&older).unwrap(),
        "perubahan tua harus dibuang"
    );
    assert_eq!(
        replica.store().get("flow-ir/1", "a").unwrap(),
        Some(json!({"v": "baru"}))
    );
}

#[test]
fn sync_replaying_the_same_changes_is_idempotent() {
    let (_src_dir, src_store) = sqlite();
    let (_rep_dir, rep_store) = sqlite();
    let source = Source::new(src_store);
    let replica = Replica::new(rep_store);
    source.put("api-ir/1", "x", &json!(1)).unwrap();
    let changes = source.changes_since(0).unwrap();
    assert_eq!(replica.apply_all(&changes).unwrap(), 1);
    assert_eq!(replica.apply_all(&changes).unwrap(), 0);
}

#[test]
fn sync_invalid_key_is_refused_by_replica() {
    let (_dir, store) = sqlite();
    let replica = Replica::new(store);
    let bad = Change {
        version: 1,
        kind: "flow-ir/1".to_owned(),
        id: String::new(),
        op: Op::Delete,
    };
    assert!(matches!(
        replica.apply(&bad),
        Err(StoreError::InvalidKey(_))
    ));
}

#[test]
fn sync_change_log_survives_reopen_and_versions_continue() {
    let dir = tempfile::tempdir().unwrap();
    let log_path = dir.path().join("changes.sqlite");
    let (_src_dir, src_store) = sqlite();
    {
        let source = Source::with_log(src_store, SqliteChangeLog::open(&log_path).unwrap());
        source.put("flow-ir/1", "a", &json!({"v": 1})).unwrap();
        source.delete("flow-ir/1", "a").unwrap();
    }
    // Proses baru membuka log yang sama: versi harus lanjut dari 2, tidak mulai dari 1.
    let (_store_dir, store) = sqlite();
    let source = Source::with_log(store, SqliteChangeLog::open(&log_path).unwrap());
    assert_eq!(source.head().unwrap(), 2);
    assert_eq!(source.put("flow-ir/1", "b", &json!(2)).unwrap(), 3);
    let all = source.changes_since(0).unwrap();
    assert_eq!(
        all.iter().map(|c| c.version).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(all[1].op, Op::Delete);
    assert_eq!(all[0].op, Op::Put(json!({"v": 1})));
}
