//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn model_dir_for(model_name: &str) -> Option<PathBuf> {
    EmbeddingEngine::resolve_model_dir(&EmbeddingConfig {
        model_name: model_name.to_string(),
        ..EmbeddingConfig::default()
    })
}

fn model_dir() -> Option<PathBuf> {
    model_dir_for("all-MiniLM-L6-v2")
}

#[tokio::test]
async fn test_e5_prefixes_apply_to_queries_and_passages() {
    assert_eq!(default_prefixes("all-MiniLM-L6-v2"), ("", ""));
    assert_eq!(
        default_prefixes("multilingual-e5-small"),
        ("query: ", "passage: ")
    );
    let config = EmbeddingConfig {
        model_name: "multilingual-e5-test".into(),
        dimension: 64,
        ..EmbeddingConfig::default()
    };
    let e5 = EmbeddingEngine::build(config, Backend::Hashing).expect("build");
    let plain = EmbeddingEngine::hashing(64).expect("plain");
    // A query is embedded as "query: <text>", a passage as "passage: <text>".
    assert_eq!(
        e5.embed("cara deploy").await.expect("q"),
        plain.embed("query: cara deploy").await.expect("plain q")
    );
    assert_eq!(
        e5.embed_batch(&["isi catatan"]).await.expect("p")[0],
        plain.embed("passage: isi catatan").await.expect("plain p")
    );
}

#[tokio::test]
async fn test_multilingual_e5_embeds_when_installed() {
    let config = EmbeddingConfig {
        model_name: "multilingual-e5-small".into(),
        dimension: 384,
        ..EmbeddingConfig::default()
    };
    let Ok(e) = EmbeddingEngine::with_config(config).await else {
        eprintln!("SKIPPED: multilingual-e5-small is not installed");
        return;
    };
    let v = e.embed("cara membuat dokumen").await.expect("embed");
    assert_eq!(v.len(), 384);
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-3, "L2-normalised, got {norm}");
}

#[test]
fn test_embedding_identity_changes_with_runtime_configuration() {
    let base = EmbeddingEngine::hashing(64).expect("base");
    let prefixed = EmbeddingEngine::build(
        EmbeddingConfig {
            model_name: "feature-hashing".into(),
            dimension: 64,
            query_prefix: Some("query: ".into()),
            ..EmbeddingConfig::default()
        },
        Backend::Hashing,
    )
    .expect("prefixed");
    let other_dim = EmbeddingEngine::hashing(65).expect("other dimension");
    assert_ne!(base.identity().unwrap(), prefixed.identity().unwrap());
    assert_ne!(base.identity().unwrap(), other_dim.identity().unwrap());
    assert_eq!(base.identity().unwrap(), base.identity().unwrap());
}

#[test]
fn test_default_config() {
    let config = EmbeddingConfig::default();
    assert_eq!(config.dimension, 384);
    assert_eq!(config.model_name, "multilingual-e5-small");
}

#[test]
fn test_cosine_similarity_identical() {
    let v = vec![1.0, 0.0, 0.0];
    let sim = cosine_similarity(&v, &v);
    assert!((sim - 1.0).abs() < 0.0001);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    let sim = cosine_similarity(&a, &b);
    assert!((sim - 0.0).abs() < 0.0001);
}

#[test]
fn test_cosine_similarity_opposite() {
    let a = vec![1.0, 0.0];
    let b = vec![-1.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert!((sim - (-1.0)).abs() < 0.0001);
}

#[test]
fn test_cosine_similarity_different_lengths() {
    let a = vec![1.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert_eq!(sim, 0.0);
}

#[test]
fn test_wordpiece_basic_and_encode() {
    let mut vocab = HashMap::new();
    for (i, t) in [
        "[PAD]", "[UNK]", "[CLS]", "[SEP]", "hello", "world", "##s", "!",
    ]
    .iter()
    .enumerate()
    {
        vocab.insert((*t).to_string(), i as i64);
    }
    let wp = WordPiece {
        vocab,
        unk: 1,
        cls: 2,
        sep: 3,
        pad: 0,
    };
    assert_eq!(wp.encode("Hello worlds!", 16), vec![2, 4, 5, 6, 7, 3]);
    assert_eq!(wp.encode("zzz", 16), vec![2, 1, 3]);
    // Truncation keeps [CLS] ... [SEP].
    assert_eq!(wp.encode("hello world hello world", 4), vec![2, 4, 5, 3]);
    assert_eq!(
        WordPiece::basic("a-b 日本"),
        vec!["a", "-", "b", "日", "本"]
    );
}

#[tokio::test]
async fn test_hashing_engine_properties() {
    let e = EmbeddingEngine::hashing(64).expect("engine");
    assert_eq!(e.backend(), EmbeddingBackend::Hashing);
    let v = e.embed("vector search engine").await.expect("embed");
    assert_eq!(v.len(), 64);
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5);
    // Deterministic.
    assert_eq!(v, e.embed("vector search engine").await.expect("again"));
    // Word overlap scores higher than none.
    let near = e.embed("vector search library").await.expect("near");
    let far = e.embed("chocolate cake recipe").await.expect("far");
    assert!(cosine_similarity(&v, &near) > cosine_similarity(&v, &far));
}

#[tokio::test]
async fn test_cache_hits_and_clear() {
    let mut e = EmbeddingEngine::hashing(32).expect("engine");
    assert_eq!(e.cache_hit_ratio(), 0.0);
    e.embed_batch(&["a b", "c d"]).await.expect("first");
    e.embed_batch(&["a b", "c d", "e f"]).await.expect("second");
    // 5 lookups: 2 hits, 3 misses.
    assert!((e.cache_hit_ratio() - 0.4).abs() < 1e-9);
    e.clear_cache();
    assert_eq!(e.cache_hit_ratio(), 0.0);
    e.embed("a b").await.expect("after clear");
    assert_eq!(e.cache_hit_ratio(), 0.0);
}

#[tokio::test]
async fn test_batch_order_and_subbatching() {
    let mut e = EmbeddingEngine::hashing(16).expect("engine");
    e.config.max_batch_size = 2;
    let texts: Vec<String> = (0..7).map(|i| format!("text number {i}")).collect();
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let batch = e.embed_batch(&refs).await.expect("batch");
    assert_eq!(batch.len(), 7);
    for (t, v) in refs.iter().zip(&batch) {
        assert_eq!(*v, hash_embed(t, 16));
    }
}

#[tokio::test]
async fn test_missing_model_is_error_not_fallback() {
    let cfg = EmbeddingConfig {
        model_name: "no-such-model".to_string(),
        model_dir: Some(PathBuf::from("definitely/missing")),
        ..EmbeddingConfig::default()
    };
    assert!(matches!(
        EmbeddingEngine::with_config(cfg).await,
        Err(MemoryError::EmbeddingModelError(_))
    ));
    let bad = EmbeddingConfig {
        dimension: 0,
        ..EmbeddingConfig::default()
    };
    assert!(matches!(
        EmbeddingEngine::with_config(bad).await,
        Err(MemoryError::ConfigError(_))
    ));
}

#[tokio::test]
async fn test_onnx_semantic_similarity() {
    let Some(dir) = model_dir() else {
        eprintln!("SKIPPED test_onnx_semantic_similarity: no model.onnx/vocab.txt found");
        return;
    };
    let cfg = EmbeddingConfig {
        model_name: "all-MiniLM-L6-v2".into(),
        model_dir: Some(dir),
        ..EmbeddingConfig::default()
    };
    let e = EmbeddingEngine::with_config(cfg).await.expect("load onnx");
    assert_eq!(e.backend(), EmbeddingBackend::Onnx);
    let v = e
        .embed_batch(&[
            "How do I find similar documents with vectors?",
            "Nearest neighbor search over embeddings",
            "Recipe for baking chocolate cake",
        ])
        .await
        .expect("embed");
    assert!(v.iter().all(|x| x.len() == 384));
    let norm: f32 = v[0].iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-3);
    let related = cosine_similarity(&v[0], &v[1]);
    let unrelated = cosine_similarity(&v[0], &v[2]);
    assert!(
        related > unrelated,
        "related={related} unrelated={unrelated}"
    );
    // Padding must not change a text's embedding: alone vs. batched with a longer text.
    let alone = e
        .embed("Recipe for baking chocolate cake")
        .await
        .expect("alone");
    let padding_similarity = cosine_similarity(&alone, &v[2]);
    assert!(
        padding_similarity > 0.99,
        "padding_similarity={padding_similarity}"
    );
}
