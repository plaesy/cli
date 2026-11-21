//! Unit tests for `tests` split out of `config.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn the_default_config_enables_only_the_zero_config_platforms() {
    let cfg = default_config();
    assert!(cfg.platforms[&Platform::Web].enabled);
    assert!(!cfg.platforms[&Platform::Twitter].enabled);
    assert_eq!(cfg.platforms.len(), Platform::ALL.len());
}

#[test]
fn a_legacy_file_converts_to_the_unified_shape() {
    let legacy = br#"{"Platforms":{"web":{"Enabled":true,"PrimaryBackend":"jina-reader","FallbackBackends":["curl"]}},"cache_dir":"x","cache_size_mb":5,"default_cache_ttl":300000000000}"#;
    let unified = migrate_legacy(legacy).expect("convert");
    let reach = unified.reach.expect("reach section");
    assert_eq!(reach.cache_dir, "x");
    assert_eq!(reach.default_cache_ttl, "5m0s");
    assert_eq!(reach.platforms["web"].default_backend, "jina-reader");
}

#[test]
fn the_legacy_and_unified_shapes_round_trip_the_backends() {
    let cfg = default_config();
    let back = from_unified(&to_unified(&cfg));
    assert_eq!(
        back.platforms[&Platform::Web].primary_backend,
        "jina-reader"
    );
    assert_eq!(
        back.platforms[&Platform::Rss].fallback_backends,
        vec!["feedparser".to_owned()]
    );
}
