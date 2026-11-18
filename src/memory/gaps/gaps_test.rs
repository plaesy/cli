//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::memory::storage::DatabaseConfig;
use chrono::Duration;

async fn db() -> Database {
    Database::new(DatabaseConfig {
        db_path: ":memory:".into(),
        ..DatabaseConfig::default()
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn weak_queries_accumulate_and_normalise() {
    let db = db().await;
    db.record_query("Deploy  Steps", None).unwrap();
    db.record_query("deploy steps", Some(0.1)).unwrap();
    let gaps = db.open_query_gaps(10, Utc::now()).unwrap();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].misses, 2);
    assert_eq!(gaps[0].query, "Deploy  Steps");
}

#[tokio::test]
async fn strong_hit_closes_and_next_miss_reopens() {
    let db = db().await;
    db.record_query("kata sandi", None).unwrap();
    db.record_query("kata sandi", Some(0.9)).unwrap();
    assert!(db.open_query_gaps(10, Utc::now()).unwrap().is_empty());
    db.record_query("kata sandi", Some(0.05)).unwrap();
    assert_eq!(db.open_query_gaps(10, Utc::now()).unwrap().len(), 1);
}

#[tokio::test]
async fn nan_score_is_a_miss_and_long_query_is_cut() {
    let db = db().await;
    db.record_query("nan case", Some(f32::NAN)).unwrap();
    let gaps = db.open_query_gaps(10, Utc::now()).unwrap();
    assert_eq!(gaps.len(), 1);
    let long = "q".repeat(MAX_QUERY_CHARS + 50);
    db.record_query(&long, None).unwrap();
    let stored = db.open_query_gaps(10, Utc::now()).unwrap();
    assert!(stored
        .iter()
        .any(|g| g.query.chars().count() == MAX_QUERY_CHARS));
}

#[tokio::test]
async fn resolve_by_id_and_priority_decays() {
    let db = db().await;
    db.record_query("old question", None).unwrap();
    db.record_query("new question", None).unwrap();
    let gaps = db.open_query_gaps(10, Utc::now()).unwrap();
    assert!(db.resolve_query_gap(gaps[0].id).unwrap());
    assert!(!db.resolve_query_gap(gaps[0].id).unwrap());
    // One half-life later the same miss counts half as much.
    let later = Utc::now() + Duration::days(HALF_LIFE_DAYS as i64);
    let now = db.open_query_gaps(10, Utc::now()).unwrap();
    let then = db.open_query_gaps(10, later).unwrap();
    assert!((then[0].priority - now[0].priority * 0.5).abs() < 0.01);
}
