//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::memory::storage::DatabaseConfig;

fn res(id: u64, conf: ConfidenceLevel) -> SearchResult {
    SearchResult {
        chunk_id: id,
        text: format!("t{id}"),
        source_path: "a.md".into(),
        start_line: 1,
        end_line: 2,
        revision: None,
        indexed_at: "2026-01-01T00:00:00Z".into(),
        score: 0.0,
        confidence: conf,
        meta: Default::default(),
        explain: None,
    }
}

async fn engine(with_vectors: bool) -> SearchEngine {
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: PathBuf::from(":memory:"),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let emb = with_vectors.then(|| Arc::new(EmbeddingEngine::hashing(128).expect("hash")));
    SearchEngine::new(db, emb, SearchConfig::default())
        .await
        .expect("engine")
}

fn write(dir: &Path, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(&p, content).expect("write");
    p
}

fn rrf_ranker() -> Ranker {
    Ranker::new(RankerConfig {
        ranker_type: RankerType::RRF,
        ..RankerConfig::default()
    })
}

#[test]
fn test_weighted_keeps_semantic_winner_over_weak_keyword_hit() {
    let r = Ranker::new(RankerConfig::default());
    let mut strong_vec = res(1, ConfidenceLevel::Inferred);
    strong_vec.score = 0.8;
    let mut weak_kw = res(2, ConfidenceLevel::Fact);
    weak_kw.score = 0.2;
    let out = r.merge_and_rank(&[strong_vec], &[weak_kw]).expect("rank");
    assert_eq!(out[0].chunk_id, 1);
    assert!((out[0].score - 0.56).abs() < 1e-5); // 0.7 * 0.8 / (0.7 + 0.3)
    assert!((out[1].score - 0.06).abs() < 1e-5); // 0.3 * 0.2
    assert_eq!(out[1].confidence, ConfidenceLevel::Fact);
    assert_eq!(out[0].confidence, ConfidenceLevel::Inferred);
}

#[test]
fn test_ranker_config_default() {
    let config = RankerConfig::default();
    assert_eq!(config.vector_weight, 0.7);
    assert_eq!(config.keyword_weight, 0.3);
    assert_eq!(config.ranker_type, RankerType::Weighted);
}

#[test]
fn test_ranker_type_serialize() {
    let rt = RankerType::RRF;
    let json = serde_json::to_string(&rt).expect("serialize");
    assert_eq!(json, "\"rrf\"");
}

#[test]
fn test_rrf_prefers_docs_in_both_lists_and_dedupes() {
    let r = rrf_ranker();
    let vec_hits = vec![
        res(1, ConfidenceLevel::Inferred),
        res(2, ConfidenceLevel::Inferred),
    ];
    let kw_hits = vec![res(2, ConfidenceLevel::Fact), res(3, ConfidenceLevel::Fact)];
    let out = r.merge_and_rank(&vec_hits, &kw_hits).expect("rank");
    let ids: Vec<u64> = out.iter().map(|x| x.chunk_id).collect();
    assert_eq!(ids, vec![2, 1, 3]);
    assert!((out[0].score - (0.7 / 62.0 + 0.3 / 61.0) / (0.7 / 61.0 + 0.3 / 61.0)).abs() < 1e-5);
    assert!(out.iter().all(|x| x.score > 0.0 && x.score <= 1.0));
    // Fact wins when keyword matched; vector-only stays Inferred.
    assert_eq!(out[0].confidence, ConfidenceLevel::Fact);
    assert_eq!(out[1].confidence, ConfidenceLevel::Inferred);
    assert_eq!(out[2].confidence, ConfidenceLevel::Fact);
}

#[test]
fn test_rrf_single_source_top_score_is_one() {
    let r = rrf_ranker();
    let out = r
        .merge_and_rank(
            &[],
            &[res(5, ConfidenceLevel::Fact), res(6, ConfidenceLevel::Fact)],
        )
        .expect("rank");
    assert!((out[0].score - 1.0).abs() < 1e-6);
    assert!(out[1].score < out[0].score);
    assert!(r.merge_and_rank(&[], &[]).expect("empty").is_empty());
}

#[test]
fn test_ranker_rejects_bad_config() {
    let bad = |vw, kw, t| {
        Ranker::new(RankerConfig {
            vector_weight: vw,
            keyword_weight: kw,
            ranker_type: t,
        })
        .merge_and_rank(&[res(1, ConfidenceLevel::Fact)], &[])
    };
    assert!(matches!(
        bad(-1.0, 1.0, RankerType::RRF),
        Err(MemoryError::RankingError(_))
    ));
    assert!(matches!(
        bad(0.0, 0.0, RankerType::RRF),
        Err(MemoryError::RankingError(_))
    ));
    assert!(matches!(
        bad(f32::NAN, 1.0, RankerType::RRF),
        Err(MemoryError::RankingError(_))
    ));
}

#[test]
fn test_citation_validation_and_display() {
    let b = CitationBuilder::new();
    let mut r = res(9, ConfidenceLevel::Fact);
    r.revision = Some("abc123".into());
    let c = b.build(&r).expect("cite");
    assert_eq!(c.confidence, "FACT");
    assert_eq!(
        c.to_string(),
        format!(
            "a.md:1-2 [FACT] indexed 2026-01-01T00:00:00Z @ abc123 {}",
            docid_for("a.md")
        )
    );
    r.start_line = 0;
    assert!(matches!(b.build(&r), Err(MemoryError::CitationError(_))));
    r.start_line = 3;
    assert!(b.build(&r).is_err());
    let mut r = res(9, ConfidenceLevel::Fact);
    r.source_path.clear();
    assert!(b.build(&r).is_err());
    let mut r = res(9, ConfidenceLevel::Inferred);
    r.indexed_at.clear();
    assert!(b.build(&r).is_err());
}

#[test]
fn test_docid_derived_from_path() {
    let id1 = docid_for("a/b.md");
    let id2 = docid_for("A/b.md"); // case-insensitive normalization
    assert_eq!(id1, id2);
    let id3 = docid_for("a/../b/c.md"); // normalized relative path
    assert!(!id1.eq(&id3));
    assert!(id1.len() == 6);
    // Same path -> same docid; different path -> different docid
    let id4 = docid_for("a/x.md");
    let id5 = docid_for("a/y.md");
    assert_ne!(id4, id5);
}

#[test]
fn test_docid_is_stable_and_path_based() {
    // First 6 hex chars of SHA-256 of normalized path
    let id_a = docid_for("notes/a.md");
    let id_a_again = docid_for("notes/a.md");
    assert_eq!(id_a, id_a_again);
    assert!(id_a.len() == 6);
    assert!(
        id_a.chars().all(|c| c.is_ascii_hexdigit()),
        "docid must be hex"
    );
    // Moving the file changes the docid (acceptable breakage per spec)
    let id_b = docid_for("notes/b.md");
    assert_ne!(id_a, id_b);
    // Backslashes normalized to forward slashes
    let id_win = docid_for(r"notes\a.md");
    assert_eq!(id_a, id_win, "backslash normalized to forward slash");
}

#[tokio::test]
async fn index_directory_removes_stale_temp_files_but_keeps_fresh_ones() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    write(root, "note.md", "# Kept\n\nReal content.\n");
    let stale = root.join(".leftover.tmp-999-123");
    std::fs::write(&stale, "orphaned by a crashed writer").expect("write stale");
    // Back-date it well past the staleness threshold.
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    let old_ft = filetime::FileTime::from_system_time(old);
    filetime::set_file_mtime(&stale, old_ft).expect("set mtime");

    let fresh = root.join(".inflight.tmp-999-456");
    std::fs::write(&fresh, "a write in progress right now").expect("write fresh");

    let e = engine(false).await;
    e.index_directory(root).await.expect("index");

    assert!(!stale.exists(), "stale temp file must be reconciled away");
    assert!(
        fresh.exists(),
        "a temp file younger than the staleness margin must not be touched"
    );
}

#[tokio::test]
async fn test_index_directory_and_search_with_exact_citations() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let content = "---\nid: EV-01\n---\n\n# Vectors\n\nHNSW graphs enable approximate nearest neighbor search.\n";
    write(root, "notes/vectors.md", content);
    write(
        root,
        "cooking.md",
        "# Cake\n\nMix flour and sugar then bake.\n",
    );
    let e = engine(true).await;
    let s = e.index_directory(root).await.expect("index");
    assert_eq!((s.indexed, s.unchanged, s.removed), (2, 0, 0));
    assert!(s.failed.is_empty());

    let hits = e
        .search("nearest neighbor graphs", 5)
        .await
        .expect("search");
    assert!(!hits.is_empty());
    let top = &hits[0];
    assert_eq!(top.source_path, "notes/vectors.md");
    assert_eq!(top.confidence, ConfidenceLevel::Fact);
    // Cited line range must reproduce the chunk text from the real file.
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(top.text, lines[top.start_line - 1..top.end_line].join("\n"));
    // EV-01: every result is citable.
    let b = CitationBuilder::new();
    for r in &hits {
        b.build(r).expect("citation");
    }
}

#[tokio::test]
async fn test_index_file_rejects_path_outside_root() {
    let root = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    let path = write(outside.path(), "secret.md", "outside secret\n");
    let e = engine(false).await;
    let err = e
        .index_file(root.path(), &path)
        .await
        .expect_err("outside path must fail");
    assert!(err.to_string().contains("outside the notes root"));
    assert!(e
        .database()
        .document_paths()
        .await
        .expect("paths")
        .is_empty());
}

#[tokio::test]
async fn test_reindex_unchanged_then_modified_replaces_old_content() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let p = write(root, "a.md", "alpha unique1 text\n");
    let e = engine(true).await;
    e.index_directory(root).await.expect("1");
    let again = e.index_directory(root).await.expect("2");
    assert_eq!((again.indexed, again.unchanged), (0, 1));

    std::fs::write(&p, "beta unique2 text\n").expect("rewrite");
    let s = e.index_directory(root).await.expect("3");
    assert_eq!(s.indexed, 1);
    assert!(e.keyword_search("unique1", 5).await.expect("kw").is_empty());
    assert_eq!(e.keyword_search("unique2", 5).await.expect("kw").len(), 1);
    // Old vector must be gone too: one chunk, one vector.
    assert_eq!(e.hnsw.read().len(), 1);
    let st = e.statistics().await.expect("stats");
    assert_eq!((st.documents_total, st.chunks_total), (1, 1));
}

#[tokio::test]
async fn test_deleted_file_is_removed_from_both_stores() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let p = write(root, "gone.md", "ephemeral content here\n");
    write(root, "stay.md", "persistent content here\n");
    let e = engine(true).await;
    e.index_directory(root).await.expect("index");
    std::fs::remove_file(&p).expect("rm");
    let s = e.index_directory(root).await.expect("resync");
    assert_eq!(s.removed, 1);
    assert!(e
        .keyword_search("ephemeral", 5)
        .await
        .expect("kw")
        .is_empty());
    assert_eq!(e.hnsw.read().len(), 1);
    assert!(!e.remove_path("gone.md").await.expect("again"));
}

#[tokio::test]
async fn forget_removes_index_and_suppresses_synced_conflict_copies() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let forgotten = "---\nid: stable-note-id\nscope: [project-a]\n---\n\n# Private\n\nsecret phrase to forget\n";
    write(root, "private.md", forgotten);
    write(root, "stay.md", "unrelated retained information\n");
    let engine = engine(true).await;
    engine.set_watch_root(root.to_path_buf());
    engine.index_directory(root).await.expect("initial index");
    assert_eq!(
        engine
            .keyword_search("secret phrase", 5)
            .await
            .expect("before forget")
            .len(),
        1
    );

    engine
        .forget_note(Path::new("private.md"), "owner-request")
        .await
        .expect("forget note");
    assert!(!root.join("private.md").exists());
    assert!(engine
        .keyword_search("secret phrase", 5)
        .await
        .expect("after forget")
        .is_empty());
    assert_eq!(engine.hnsw.read().len(), 1);

    // A sync conflict copy retains the stable id and must not re-enter retrieval.
    write(root, "private.sync-conflict.md", forgotten);
    engine
        .index_directory(root)
        .await
        .expect("resync conflict copy");
    assert!(engine
        .keyword_search("secret phrase", 5)
        .await
        .expect("after conflict sync")
        .is_empty());
    assert_eq!(engine.statistics().await.expect("stats").documents_total, 1);
}

#[tokio::test]
async fn update_is_conflict_checked_audited_and_reindexed() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let original =
        "---\nid: stable-note-id\nscope: [project-a]\n---\n\n# Claim\n\nRetiredParachute.\n";
    let revised =
        "---\nid: stable-note-id\nscope: [project-a]\n---\n\n# Claim\n\nUpdatedSeashell.\n";
    write(root, "claim.md", original);
    let engine = engine(true).await;
    engine.set_watch_root(root.to_path_buf());
    engine.index_directory(root).await.expect("initial index");

    let reviewed_hash = engine
        .fingerprint_note(Path::new("claim.md"))
        .expect("fingerprint");
    assert_eq!(
        reviewed_hash,
        crate::memory::indexer::content_hash(original)
    );
    let updated_hash = engine
        .update_note(
            Path::new("claim.md"),
            &reviewed_hash,
            revised,
            "new-evidence",
        )
        .await
        .expect("update");
    assert_eq!(updated_hash, crate::memory::indexer::content_hash(revised));
    assert_eq!(
        std::fs::read_to_string(root.join("claim.md")).unwrap(),
        revised
    );
    assert!(engine
        .keyword_search("RetiredParachute", 5)
        .await
        .expect("old result")
        .is_empty());
    assert_eq!(
        engine
            .keyword_search("UpdatedSeashell", 5)
            .await
            .expect("new result")
            .len(),
        1
    );

    let events = root.join(".plaesy-memory").join("events");
    let entries: Vec<_> = std::fs::read_dir(events)
        .unwrap()
        .collect::<std::io::Result<_>>()
        .unwrap();
    assert_eq!(entries.len(), 2);
    let trace = entries
        .iter()
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
        .collect::<String>();
    assert!(trace.contains(&reviewed_hash));
    assert!(trace.contains(&updated_hash));
    assert!(trace.contains("new-evidence"));
    assert!(!trace.contains("UpdatedSeashell"));
}

#[tokio::test]
async fn update_rejects_a_changed_source_and_a_changed_stable_id() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let original = "---\nid: stable-note-id\n---\n\n# Claim\n\nOriginal.\n";
    write(root, "claim.md", original);
    let engine = engine(true).await;
    engine.set_watch_root(root.to_path_buf());
    let reviewed_hash = engine
        .fingerprint_note(Path::new("claim.md"))
        .expect("fingerprint");

    let err = engine
        .update_note(
            Path::new("claim.md"),
            "short",
            "invalid-hash replacement",
            "correction",
        )
        .await
        .expect_err("malformed fingerprints must be rejected");
    assert!(err.to_string().contains("64-character SHA-256"));
    assert_eq!(
        std::fs::read_to_string(root.join("claim.md")).unwrap(),
        original
    );

    let invalid_frontmatter_hash = engine
        .fingerprint_note(Path::new("claim.md"))
        .expect("fingerprint unchanged note");
    let err = engine
        .update_note(
            Path::new("claim.md"),
            &invalid_frontmatter_hash,
            "---\nid: stable-note-id\ninvalid: [yaml\n---\n\nBody.\n",
            "correction",
        )
        .await
        .expect_err("invalid YAML front matter must be rejected");
    assert!(err.to_string().contains("YAML parse error"));
    assert_eq!(
        std::fs::read_to_string(root.join("claim.md")).unwrap(),
        original
    );

    write(root, "claim.md", "external concurrent edit\n");
    let err = engine
        .update_note(
            Path::new("claim.md"),
            &reviewed_hash,
            "my stale replacement",
            "correction",
        )
        .await
        .expect_err("stale fingerprint must be rejected");
    assert!(err.to_string().contains("changed since it was reviewed"));
    assert_eq!(
        std::fs::read_to_string(root.join("claim.md")).unwrap(),
        "external concurrent edit\n"
    );

    let current_hash = engine
        .fingerprint_note(Path::new("claim.md"))
        .expect("new fingerprint");
    let err = engine
        .update_note(
            Path::new("claim.md"),
            &current_hash,
            "---\nid: different-id\n---\n\nReplacement.\n",
            "correction",
        )
        .await
        .expect_err("stable identity must be preserved");
    assert!(err.to_string().contains("stable id"));
    assert_eq!(
        std::fs::read_to_string(root.join("claim.md")).unwrap(),
        "external concurrent edit\n"
    );
}

#[tokio::test]
async fn archive_marks_note_inactive_without_deleting_it_and_records_hash_only_event() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let original = "\u{feff}---\n# preserve top-level note comments\nid: stable-note-id\nstatus: \"active # scalar value\" # preserve status note\nscope: [project-a]\n---\n\nArchiveMeadow.\n";
    write(root, "notes/claim.md", original);
    let engine = engine(true).await;
    engine.set_watch_root(root.to_path_buf());
    engine.index_directory(root).await.expect("initial index");
    let reviewed_hash = engine
        .fingerprint_note(Path::new("notes/claim.md"))
        .expect("fingerprint");

    let archived_hash = engine
        .archive_note(Path::new("notes/claim.md"), &reviewed_hash, "completed")
        .await
        .expect("archive");
    let archived_content =
        std::fs::read_to_string(root.join("notes/claim.md")).expect("archived source remains");
    assert!(archived_content.starts_with("\u{feff}---"));
    assert!(archived_content.contains("status: archived"));
    assert!(archived_content.contains("# preserve top-level note comments"));
    assert!(archived_content.contains("status: archived # preserve status note"));
    assert_ne!(archived_hash, reviewed_hash);
    assert!(engine
        .keyword_search("ArchiveMeadow", 5)
        .await
        .expect("default retrieval")
        .is_empty());

    let include_inactive = SearchFilter {
        include_inactive: true,
        ..SearchFilter::default()
    };
    assert_eq!(
        engine
            .search_with("ArchiveMeadow", 5, SearchMode::Keyword, &include_inactive,)
            .await
            .expect("include archived")
            .len(),
        1
    );

    let event_dir = root.join(".plaesy-memory").join("events");
    let trace = std::fs::read_dir(event_dir)
        .expect("event directory")
        .map(|entry| std::fs::read_to_string(entry.expect("event").path()).unwrap())
        .collect::<String>();
    assert!(trace.contains("\"operation\":\"archive\""));
    assert!(trace.contains(&reviewed_hash));
    assert!(trace.contains(&archived_hash));
    assert!(trace.contains("completed-work"));
    assert!(!trace.contains("ArchiveMeadow"));

    let error = engine
        .archive_note(Path::new("notes/claim.md"), &archived_hash, "completed")
        .await
        .expect_err("an archived note must not be archived twice");
    assert!(error.to_string().contains("already archived"));

    let restored_content =
        "---\nid: stable-note-id\nstatus: active\nscope: [project-a]\n---\n\nArchiveMeadow.\n";
    let restored_hash = engine
        .update_note(
            Path::new("notes/claim.md"),
            &archived_hash,
            restored_content,
            "correction",
        )
        .await
        .expect("restore through reviewed update");
    assert_ne!(restored_hash, archived_hash);
    assert_eq!(
        engine
            .keyword_search("ArchiveMeadow", 5)
            .await
            .expect("restored note search")
            .len(),
        1
    );
}

#[tokio::test]
async fn test_bad_file_does_not_abort_sync() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    write(root, "empty.md", "  \n");
    write(root, "good.md", "fine content\n");
    // Broken frontmatter must not make the file unsearchable.
    write(
        root,
        "tmpl.md",
        "---\nid: {{ID}}: x\n---\nplaceholder body\n",
    );
    let e = engine(false).await;
    let s = e.index_directory(root).await.expect("index");
    assert_eq!(s.indexed, 2);
    // Berkas kosong dilewati (tidak ada teks untuk diindeks), bukan dihitung sebagai gagal.
    assert_eq!(s.skipped_empty, 1);
    assert!(s.failed.is_empty(), "failed: {:?}", s.failed);
    let hits = e.keyword_search("placeholder", 5).await.expect("kw");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source_path, "tmpl.md");
    // Line numbers refer to the real file (body is on line 4).
    assert_eq!((hits[0].start_line, hits[0].end_line), (1, 4));
}

#[tokio::test]
async fn test_semantic_search_and_keyword_only_mode() {
    let dir = tempfile::tempdir().expect("tmp");
    write(dir.path(), "a.md", "vector similarity search\n");
    let sem = engine(true).await;
    sem.index_directory(dir.path()).await.expect("index");
    let hits = sem
        .semantic_search("vector similarity", 3)
        .await
        .expect("sem");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].confidence, ConfidenceLevel::Inferred);
    assert!(hits[0].score > 0.5);

    let kw = engine(false).await;
    kw.index_directory(dir.path()).await.expect("index");
    assert!(!kw.has_vector_search());
    assert!(matches!(
        kw.semantic_search("vector", 3).await,
        Err(MemoryError::FeatureNotEnabled { .. })
    ));
    assert_eq!(kw.search("vector", 3).await.expect("hybrid").len(), 1);
}

#[tokio::test]
async fn test_hybrid_exact_match_is_fact_with_top_score() {
    let dir = tempfile::tempdir().expect("tmp");
    write(dir.path(), "a.md", "rust memory safety\n");
    let e = engine(true).await;
    e.index_directory(dir.path()).await.expect("index");
    let hits = e.search("rust memory safety", 5).await.expect("hybrid");
    assert_eq!(hits[0].confidence, ConfidenceLevel::Fact);
    assert!(hits[0].score >= 0.65, "score {}", hits[0].score);
}

#[tokio::test]
async fn test_onnx_semantic_only_hit_is_inferred() {
    let cfg = crate::memory::embeddings::EmbeddingConfig::default();
    let Ok(emb) = EmbeddingEngine::with_config(cfg).await else {
        eprintln!("SKIPPED test_onnx_semantic_only_hit_is_inferred: no model");
        return;
    };
    let dir = tempfile::tempdir().expect("tmp");
    write(
        dir.path(),
        "nn.md",
        "Nearest neighbor lookup over embeddings\n",
    );
    write(
        dir.path(),
        "cake.md",
        "Baking a chocolate cake with flour\n",
    );
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: PathBuf::from(":memory:"),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let e = SearchEngine::new(db, Some(Arc::new(emb)), SearchConfig::default())
        .await
        .expect("engine");
    e.index_directory(dir.path()).await.expect("index");
    // No query word occurs in either file: only the vector leg can match.
    let hits = e
        .search("finding similar vectors quickly", 2)
        .await
        .expect("hybrid");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].source_path, "nn.md");
    assert_eq!(hits[0].confidence, ConfidenceLevel::Inferred);
}

#[tokio::test]
async fn test_limits_empty_query_and_threshold() {
    let dir = tempfile::tempdir().expect("tmp");
    for i in 0..8 {
        write(
            dir.path(),
            &format!("d{i}.md"),
            &format!("shared token number{i}\n"),
        );
    }
    let e = engine(false).await;
    e.index_directory(dir.path()).await.expect("index");
    assert_eq!(e.search("shared", 3).await.expect("3").len(), 3);
    assert_eq!(e.search("shared", 0).await.expect("0 clamps to 1").len(), 1);
    assert!(e.search("   ", 5).await.expect("empty").is_empty());
    assert!(e.search("zzzzqqq", 5).await.expect("none").is_empty());
    let mut strict = engine(false).await;
    strict.config.confidence_threshold = 1.1;
    strict.index_directory(dir.path()).await.expect("index");
    assert!(strict.search("shared", 5).await.expect("thr").is_empty());
}

#[tokio::test]
async fn test_no_search_method_enabled_is_error() {
    let mut e = engine(false).await;
    e.config.enable_keyword_search = false;
    assert!(matches!(
        e.search("x", 3).await,
        Err(MemoryError::KeywordSearchError(_))
    ));
}

#[tokio::test]
async fn test_hnsw_rebuilt_from_database_on_new() {
    let dir = tempfile::tempdir().expect("tmp");
    let docs = tempfile::tempdir().expect("docs");
    write(docs.path(), "a.md", "persistent vector content\n");
    let cfg = DatabaseConfig {
        db_path: dir.path().join("state.db"),
        ..DatabaseConfig::default()
    };
    {
        let db = Arc::new(Database::new(cfg.clone()).await.expect("db"));
        let emb = Some(Arc::new(EmbeddingEngine::hashing(128).expect("h")));
        let e = SearchEngine::new(db, emb, SearchConfig::default())
            .await
            .expect("e");
        e.index_directory(docs.path()).await.expect("index");
    }
    let db = Arc::new(Database::new(cfg).await.expect("reopen"));
    let emb = Some(Arc::new(EmbeddingEngine::hashing(128).expect("h")));
    let e = SearchEngine::new(db, emb, SearchConfig::default())
        .await
        .expect("e2");
    assert_eq!(e.hnsw.read().len(), 1);
    assert_eq!(
        e.semantic_search("persistent vector", 3)
            .await
            .expect("sem")
            .len(),
        1
    );
}

#[tokio::test]
async fn test_search_expanded_adds_linked_passage_after_direct_hits() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    write(
        root,
        "a.md",
        "rollback procedure for the deploy\n\nsee [[b]] for the steps\n",
    );
    write(root, "b.md", "rollback steps: tag the previous build\n");
    write(root, "c.md", "unrelated gardening notes\n");
    let e = engine(false).await;
    e.index_directory(root).await.expect("index");

    let plain = e
        .search_with("rollback", 1, SearchMode::Keyword, &SearchFilter::default())
        .await
        .expect("plain");
    assert_eq!(plain.len(), 1);

    let expanded = e
        .search_expanded("rollback", 1, SearchMode::Keyword, &SearchFilter::default())
        .await
        .expect("expanded");
    // The direct hit keeps its place; the other note of the link pair follows it.
    assert_eq!(expanded.len(), 2, "direct hit plus linked passage");
    assert_eq!(expanded[0].source_path, plain[0].source_path);
    let other = if plain[0].source_path.ends_with("a.md") {
        "b.md"
    } else {
        "a.md"
    };
    assert!(
        expanded[1].source_path.ends_with(other),
        "{:?}",
        expanded[1].source_path
    );
    assert!(
        expanded[1].score < expanded[0].score,
        "linked passage ranks below direct hits"
    );
    // Its evidence label is its own: its text matched the keyword leg.
    assert!(matches!(
        expanded[1].confidence,
        crate::memory::storage::ConfidenceLevel::Fact
    ));
    // Notes with no link and no match stay out.
    assert!(expanded.iter().all(|r| !r.source_path.ends_with("c.md")));

    let explained = e
        .search_expanded_with_explain(
            "rollback",
            1,
            SearchMode::Keyword,
            &SearchFilter::default(),
            true,
        )
        .await
        .expect("explained");
    assert_eq!(explained.len(), 2);
    assert!(explained.iter().all(|result| result.explain.is_some()));
}

#[tokio::test]
async fn test_apply_events_and_health() {
    let dir = tempfile::tempdir().expect("tmp");
    let root = dir.path();
    let p = write(root, "n.md", "event driven content\n");
    let e = engine(false).await;
    let added = FileEvent {
        path: p.clone(),
        kind: EventKind::Added,
    };
    assert_eq!(e.apply_events(root, &[added]).await, 1);
    assert_eq!(e.keyword_search("driven", 5).await.expect("kw").len(), 1);
    let h = e.update_health(root).await.expect("health");
    assert_eq!(h.coverage_percent, 100.0);
    assert_eq!(h.documents_total, 1);

    // `statistics` reports the same coverage once the watch root is known, and drops
    // when a file on disk was never indexed.
    e.set_watch_root(root.to_path_buf());
    assert_eq!(e.statistics().await.expect("stats").coverage_percent, 100.0);
    write(root, "not-indexed.md", "on disk only\n");
    assert_eq!(e.statistics().await.expect("stats").coverage_percent, 50.0);

    std::fs::remove_file(&p).expect("rm");
    let del = FileEvent {
        path: p,
        kind: EventKind::Deleted,
    };
    assert_eq!(e.apply_events(root, &[del]).await, 1);
    assert!(e.keyword_search("driven", 5).await.expect("kw").is_empty());
}

#[tokio::test]
async fn test_metrics_recorded_by_search() {
    let e = engine(true).await;
    e.search("anything", 3).await.expect("search");
    e.keyword_search("anything", 3).await.expect("kw");
    let snap = e.metrics().snapshot();
    assert_eq!(snap.total_queries, 2);
    assert_eq!(snap.vector_search_success_rate, 1.0);
}
