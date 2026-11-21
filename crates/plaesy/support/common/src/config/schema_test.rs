//! Unit tests for `tests` split out of `schema.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn defaults_match_the_shipped_values() {
    let cfg = UnifiedConfig::defaults();
    assert_eq!(cfg.version, 1);
    let search = cfg.search.unwrap();
    assert_eq!(search.embedder.dimension, 768);
    assert_eq!(search.chunker.max_chunk_tokens, 256);
    assert_eq!(search.retriever.hybrid.rrf_k, 60);
    let reach = cfg.reach.unwrap();
    assert_eq!(reach.rate_limits["github"].requests, 100);
    assert_eq!(reach.timeouts["exa"], "30s");
    assert_eq!(cfg.graph.unwrap().max_file_size, 10 * 1024 * 1024);
}

#[test]
fn missing_fields_stay_zero_like_the_go_loader() {
    let cfg: UnifiedConfig = serde_json::from_str(r#"{"version": 2}"#).unwrap();
    assert_eq!(cfg.version, 2);
    assert!(cfg.search.is_none());
}

#[test]
fn type_keyword_is_written_as_type() {
    let json = serde_json::to_string(&SearchConfig::defaults().embedder).unwrap();
    assert!(json.contains(r#""type":"rembed""#), "{json}");
}

#[test]
fn round_trips_through_json() {
    let cfg = UnifiedConfig::defaults();
    let json = serde_json::to_string_pretty(&cfg).unwrap();
    let back: UnifiedConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back, cfg);
}
