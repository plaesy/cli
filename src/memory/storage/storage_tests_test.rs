//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use std::collections::HashMap;

fn doc(path: &str, body: &str) -> Document {
    Document {
        id: None,
        source_path: PathBuf::from(path),
        hash: Some("h1".into()),
        metadata: HashMap::new(),
        body: body.into(),
        frontmatter_raw: None,
        body_start_line: 1,
        chunks: Vec::new(),
        indexed_at: None,
    }
}

fn chunk(text: &str, start: usize, end: usize) -> Chunk {
    Chunk {
        id: None,
        document_id: None,
        text: text.into(),
        kind: ChunkKind::Detail,
        layer: 1,
        start_line: start,
        end_line: end,
        metadata: None,
    }
}

async fn mem_db() -> Database {
    let cfg = DatabaseConfig {
        db_path: PathBuf::from(":memory:"),
        ..DatabaseConfig::default()
    };
    Database::new(cfg).await.expect("open db")
}

#[tokio::test]
async fn list_documents_filters_counts_and_paginates_without_bodies() {
    let db = mem_db().await;
    for (path, kind) in [
        ("notes/b.md", "decision"),
        ("notes/a.md", "decision"),
        ("other/c.md", "context"),
    ] {
        let mut document = doc(path, "body must not be returned");
        document
            .metadata
            .insert("type".into(), serde_json::json!(kind));
        db.insert_document(&document)
            .await
            .expect("insert document");
    }

    assert_eq!(
        db.count_documents(Some("notes/"), Some("DECISION"), None, None)
            .await
            .expect("count"),
        2
    );
    assert_eq!(
        db.count_documents(Some("notes/"), Some("DECISION"), None, Some("BODY MUST"))
            .await
            .expect("query count"),
        2
    );
    assert_eq!(
        db.count_documents(Some("notes/"), Some("DECISION"), None, Some("no match"))
            .await
            .expect("empty query count"),
        0
    );
    let body_filtered = db
        .list_documents(
            Some("notes/"),
            Some("decision"),
            None,
            Some("BODY MUST"),
            5,
            0,
        )
        .await
        .expect("body query");
    assert_eq!(body_filtered.len(), 2);
    let page = db
        .list_documents(Some("notes/"), Some("decision"), None, None, 1, 1)
        .await
        .expect("list");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].path, "notes/b.md");
    assert_eq!(
        page[0]
            .metadata
            .get("type")
            .and_then(serde_json::Value::as_str),
        Some("decision")
    );
}

#[test]
fn test_hnsw_config_default() {
    let config = HnswConfig::default();
    assert_eq!(config.m, 32);
    assert_eq!(config.ef_construct, 256);
    assert_eq!(config.ef_search, 128);
}

#[test]
fn test_database_config_default() {
    let config = DatabaseConfig::default();
    assert!(config.wal_mode);
    assert!(config.enable_fts);
}

#[test]
fn test_confidence_level_serialize() {
    let conf = ConfidenceLevel::Fact;
    let json = serde_json::to_string(&conf).expect("serialize");
    assert_eq!(json, "\"Fact\"");
}

#[test]
fn test_fts_query_sanitizes() {
    assert_eq!(
        fts_query("hello, world"),
        Some("\"hello\" OR \"world\"".into())
    );
    // Operators in user input are inert (quoted); stopwords like "a"/"OR" are dropped.
    assert_eq!(fts_query("a\" OR NEAR("), Some("\"NEAR\"".into()));
    assert_eq!(
        fts_query("cite the SOURCE of results"),
        Some("\"cite\" OR \"SOURCE\" OR \"results\"".into())
    );
    // Stopword-only queries are kept rather than emptied.
    assert_eq!(fts_query("the of"), Some("\"the\" OR \"of\"".into()));
    assert_eq!(fts_query("  !!! "), None);
}

#[tokio::test]
async fn test_model_change_clears_hashes_only_when_it_changes() {
    let db = mem_db().await;
    // Empty database: nothing to re-embed, the model is simply recorded.
    assert!(!db
        .record_embedding_model("multilingual-e5-small")
        .await
        .expect("first"));
    assert!(!db
        .record_embedding_model("multilingual-e5-small")
        .await
        .expect("same"));

    // A document is stored with a hash; switching the model clears that hash.
    db.insert_document(&doc(".plaesy/a.md", "body"))
        .await
        .expect("doc");
    assert!(db
        .record_embedding_model("all-MiniLM-L6-v2")
        .await
        .expect("switch"));
    let hashes: i64 = db
        .conn
        .lock()
        .query_row(
            "SELECT COUNT(*) FROM documents WHERE hash IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(
        hashes, 0,
        "documents must be re-embedded after a model change"
    );
}

#[tokio::test]
async fn test_legacy_database_is_treated_as_minilm() {
    let db = mem_db().await;
    db.insert_document(&doc(".plaesy/old.md", "body"))
        .await
        .expect("doc");
    // Built before model recording: switching to E5 is a change, staying on MiniLM is not.
    assert!(!db
        .record_embedding_model(LEGACY_EMBEDDING_MODEL)
        .await
        .expect("same"));
    assert!(db
        .record_embedding_model("multilingual-e5-small")
        .await
        .expect("change"));
}

#[tokio::test]
async fn test_insert_and_fts_search_with_citation() {
    let db = mem_db().await;
    let d = doc(".plaesy/a.md", "body");
    let did = db.insert_document(&d).await.expect("doc");
    let ids = db
        .insert_chunks(
            did,
            &[
                chunk("vector embeddings for search", 3, 5),
                chunk("unrelated cooking text", 6, 8),
            ],
        )
        .await
        .expect("chunks");
    assert_eq!(ids.len(), 2);

    let hits = db.search_fts("embeddings", 10).await.expect("fts");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk_id, ids[0]);
    assert_eq!(hits[0].source_path, ".plaesy/a.md");
    assert_eq!((hits[0].start_line, hits[0].end_line), (3, 5));
    assert_eq!(hits[0].confidence, ConfidenceLevel::Fact);
    assert!(hits[0].score > 0.0 && hits[0].score < 1.0);
    assert!(!hits[0].indexed_at.is_empty());
}

#[tokio::test]
async fn test_fts_rejects_operator_injection() {
    let db = mem_db().await;
    let did = db.insert_document(&doc("a.md", "b")).await.expect("doc");
    db.insert_chunks(did, &[chunk("plain text", 1, 1)])
        .await
        .expect("chunks");
    // Would be an FTS5 syntax error if passed through raw.
    assert!(db.search_fts("\"unbalanced AND (", 5).await.is_ok());
    assert!(db.search_fts("", 5).await.expect("empty").is_empty());
}

#[tokio::test]
async fn test_reinsert_document_replaces_chunks_and_fts() {
    let db = mem_db().await;
    let did = db.insert_document(&doc("a.md", "v1")).await.expect("doc");
    db.insert_chunks(did, &[chunk("oldterm here", 1, 1)])
        .await
        .expect("c");
    let did2 = db.insert_document(&doc("a.md", "v2")).await.expect("doc2");
    assert_eq!(did, did2);
    assert!(db.get_document_chunks(did).await.expect("get").is_empty());
    assert!(db.search_fts("oldterm", 5).await.expect("fts").is_empty());
    let st = db.statistics().await.expect("stats");
    assert_eq!((st.documents_total, st.chunks_total), (1, 0));
}

#[tokio::test]
async fn test_delete_document_cascades() {
    let db = mem_db().await;
    let did = db.insert_document(&doc("a.md", "b")).await.expect("doc");
    let ids = db
        .insert_chunks(did, &[chunk("findme", 1, 1)])
        .await
        .expect("c");
    db.delete_document(did).await.expect("del");
    assert!(db.get_chunk(ids[0]).await.expect("get").is_none());
    assert!(db.search_fts("findme", 5).await.expect("fts").is_empty());
    assert!(db.find_document("a.md").await.expect("find").is_none());
}

#[tokio::test]
async fn test_embeddings_roundtrip_and_length_mismatch() {
    let db = mem_db().await;
    let did = db.insert_document(&doc("a.md", "b")).await.expect("doc");
    let ids = db.insert_chunks(did, &[chunk("x", 1, 1)]).await.expect("c");
    db.update_chunk_embeddings(&ids, &[vec![0.5, -1.0, 2.0]])
        .await
        .expect("emb");
    let all = db.all_embeddings().await.expect("all");
    assert_eq!(all, vec![(ids[0], vec![0.5, -1.0, 2.0])]);
    assert!(db.update_chunk_embeddings(&ids, &[]).await.is_err());
}

#[tokio::test]
async fn test_transaction_rollback() {
    let db = mem_db().await;
    db.begin_transaction().await.expect("begin");
    db.insert_document(&doc("a.md", "b")).await.expect("doc");
    db.rollback().await.expect("rollback");
    assert_eq!(db.statistics().await.expect("stats").documents_total, 0);
}

#[tokio::test]
async fn test_persistent_db_reopens() {
    let dir = tempfile::tempdir().expect("tmp");
    let cfg = DatabaseConfig {
        db_path: dir.path().join("sub").join("state.db"),
        ..DatabaseConfig::default()
    };
    {
        let db = Database::new(cfg.clone()).await.expect("open");
        let did = db.insert_document(&doc("a.md", "b")).await.expect("doc");
        db.insert_chunks(did, &[chunk("persisted", 1, 1)])
            .await
            .expect("c");
    }
    let db = Database::new(cfg).await.expect("reopen");
    assert_eq!(db.search_fts("persisted", 5).await.expect("fts").len(), 1);
}

#[test]
fn test_hnsw_insert_search_remove() {
    let mut idx = HnswIndex::with_dimensions(HnswConfig::default(), 3).expect("idx");
    idx.insert(1, &vec![1.0, 0.0, 0.0]).expect("i1");
    idx.insert(2, &vec![0.0, 1.0, 0.0]).expect("i2");
    idx.insert(3, &vec![0.9, 0.1, 0.0]).expect("i3");
    let r = idx.search(&vec![1.0, 0.0, 0.0], 2).expect("search");
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].0, 1);
    assert_eq!(r[1].0, 3);
    assert!(r[0].1 < r[1].1);
    assert!(idx.remove(1).expect("rm"));
    assert!(!idx.remove(1).expect("rm again"));
    assert_eq!(idx.search(&vec![1.0, 0.0, 0.0], 1).expect("s")[0].0, 3);
    assert_eq!(idx.statistics().nodes_total, 2);
}

#[test]
fn test_hnsw_dimension_mismatch() {
    let mut idx = HnswIndex::with_dimensions(HnswConfig::default(), 3).expect("idx");
    assert!(matches!(
        idx.insert(1, &vec![1.0]),
        Err(MemoryError::DimensionMismatch {
            expected: 3,
            got: 1
        })
    ));
    assert!(idx.search(&vec![1.0], 1).is_err());
}

#[test]
fn test_hnsw_grows_past_initial_capacity() {
    let mut idx = HnswIndex::with_dimensions(HnswConfig::default(), 4).expect("idx");
    for i in 0..2500u64 {
        idx.insert(
            i,
            &vec![(i % 7) as f32 + 1.0, (i % 11) as f32, 1.0, (i % 3) as f32],
        )
        .expect("ins");
    }
    assert_eq!(idx.len(), 2500);
}

#[tokio::test]
async fn pending_index_outbox_is_durable_and_idempotent() {
    let dir = tempfile::tempdir().expect("tmp");
    let db = Database::new(DatabaseConfig {
        db_path: dir.path().join("memory.sqlite"),
        ..DatabaseConfig::default()
    })
    .await
    .expect("db");
    db.enqueue_index("inbox/a.md").await.expect("enqueue");
    db.enqueue_index("inbox/a.md")
        .await
        .expect("idempotent enqueue");
    db.enqueue_index("inbox/b.md").await.expect("enqueue b");
    drop(db);
    let db = Database::new(DatabaseConfig {
        db_path: dir.path().join("memory.sqlite"),
        ..DatabaseConfig::default()
    })
    .await
    .expect("reopen db");
    assert_eq!(
        db.pending_indexes().await.expect("pending after reopen"),
        vec!["inbox/a.md", "inbox/b.md"]
    );
    db.dequeue_index("inbox/a.md").await.expect("dequeue");
    assert_eq!(
        db.pending_indexes().await.expect("pending"),
        vec!["inbox/b.md"]
    );
}

#[tokio::test]
async fn test_hnsw_save_load_roundtrip() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = dir.path().join("v.usearch");
    let mut idx = HnswIndex::with_dimensions(HnswConfig::default(), 3).expect("idx");
    idx.insert(7, &vec![0.0, 0.0, 1.0]).expect("i");
    idx.save(&path).expect("save");
    let loaded = HnswIndex::load(&path).expect("load");
    assert_eq!(loaded.dimensions(), 3);
    assert_eq!(loaded.search(&vec![0.0, 0.0, 1.0], 1).expect("s")[0].0, 7);
}
