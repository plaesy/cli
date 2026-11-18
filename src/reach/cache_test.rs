//! Unit tests for `tests` split out of `cache.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn result(data: &str) -> BackendResult {
    let mut r = BackendResult::new("test", Platform::Web);
    r.success = true;
    r.data = data.to_owned();
    r
}

#[test]
fn memory_cache_returns_what_was_stored_and_counts_hits() {
    let cache = MemoryCache::new(10, Duration::from_secs(60));
    cache.set("k", &result("v"), Duration::ZERO).expect("set");
    assert_eq!(cache.get("k").map(|r| r.data), Some("v".to_owned()));
    assert!(cache.get("missing").is_none());
    let stats = cache.stats();
    assert_eq!((stats.hits, stats.misses, stats.entries), (1, 1, 1));
}

#[test]
fn a_full_memory_cache_evicts_the_oldest_entry() {
    let cache = MemoryCache::new(1, Duration::from_secs(60));
    cache.set("a", &result("1"), Duration::ZERO).expect("set");
    cache.set("b", &result("2"), Duration::ZERO).expect("set");
    assert!(cache.get("a").is_none());
    assert!(cache.get("b").is_some());
}

#[test]
fn file_cache_survives_a_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = FileCache::new(dir.path(), 10, Duration::from_secs(60)).expect("open");
    cache
        .set("k", &result("persisted"), Duration::ZERO)
        .expect("set");
    let reopened = FileCache::new(dir.path(), 10, Duration::from_secs(60)).expect("reopen");
    assert_eq!(
        reopened.get("k").map(|r| r.data),
        Some("persisted".to_owned())
    );
}

#[test]
fn cache_keys_do_not_depend_on_option_insertion_order() {
    let mut a = super::super::types::Options::new();
    a.insert("x".into(), "1".into());
    a.insert("y".into(), "2".into());
    let mut b = super::super::types::Options::new();
    b.insert("y".into(), "2".into());
    b.insert("x".into(), "1".into());
    assert_eq!(
        cache_key(Platform::Web, "", "q", &a),
        cache_key(Platform::Web, "", "q", &b)
    );
}
