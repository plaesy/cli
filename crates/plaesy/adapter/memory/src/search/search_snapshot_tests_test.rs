//! Unit tests for `snapshot_tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::memory::storage::DatabaseConfig;

async fn open(db_path: &Path) -> SearchEngine {
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: db_path.to_path_buf(),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let emb = Some(Arc::new(EmbeddingEngine::hashing(64).expect("hash")));
    SearchEngine::new(db, emb, SearchConfig::default())
        .await
        .expect("engine")
}

#[tokio::test]
async fn snapshot_is_written_reused_and_invalidated_by_changes() {
    let docs = tempfile::tempdir().expect("docs");
    std::fs::write(docs.path().join("a.md"), "alpha snapshot content\n").expect("w");
    let store = tempfile::tempdir().expect("store");
    let db_path = store.path().join("state.db");
    let snap = store.path().join("state.db.hnsw");

    let e = open(&db_path).await;
    assert_eq!(
        e.snapshot_generation.load(Ordering::Relaxed),
        u64::MAX,
        "fresh db has no snapshot"
    );
    e.index_directory(docs.path()).await.expect("index");
    assert!(snap.exists(), "index_directory must write a snapshot");
    let gen_after_index = e.db.generation().await.expect("gen");
    assert_eq!(
        e.snapshot_generation.load(Ordering::Relaxed),
        gen_after_index
    );
    drop(e);

    // Reopen: snapshot is valid, so it is adopted (generation recorded, no rebuild needed).
    let e2 = open(&db_path).await;
    assert_eq!(
        e2.snapshot_generation.load(Ordering::Relaxed),
        gen_after_index,
        "valid snapshot is reused on reopen; startup must not rebuild HNSW"
    );
    assert_eq!(
        e2.hnsw.read().len(),
        1,
        "reused HNSW retains indexed vector"
    );
    assert_eq!(
        e2.semantic_search("alpha snapshot", 3)
            .await
            .expect("sem")
            .len(),
        1
    );

    // A change makes the next snapshot newer; a stale snapshot must never be trusted.
    std::fs::write(docs.path().join("b.md"), "beta other text\n").expect("w");
    e2.index_directory(docs.path()).await.expect("index2");
    drop(e2);
    std::fs::write(snap.with_extension("hnsw.gen"), "0").expect("corrupt gen");
    let e3 = open(&db_path).await;
    assert_eq!(
        e3.snapshot_generation.load(Ordering::Relaxed),
        u64::MAX,
        "stale gen => rebuild"
    );
    assert_eq!(
        e3.hnsw.read().len(),
        2,
        "rebuilt index must hold both documents' vectors"
    );
}

#[tokio::test]
async fn corrupt_snapshot_falls_back_to_rebuild() {
    let docs = tempfile::tempdir().expect("docs");
    std::fs::write(docs.path().join("a.md"), "gamma content\n").expect("w");
    let store = tempfile::tempdir().expect("store");
    let db_path = store.path().join("state.db");
    let e = open(&db_path).await;
    e.index_directory(docs.path()).await.expect("index");
    drop(e);
    std::fs::write(store.path().join("state.db.hnsw"), b"garbage").expect("corrupt");
    let e2 = open(&db_path).await;
    assert_eq!(e2.hnsw.read().len(), 1);
    assert_eq!(e2.snapshot_generation.load(Ordering::Relaxed), u64::MAX);
}
