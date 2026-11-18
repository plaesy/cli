//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::memory::config::SearchConfig;
use crate::memory::storage::{Database, DatabaseConfig};
use std::sync::Arc;

async fn engine_with_doc() -> (SearchEngine, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
        dir.path().join("a.md"),
        "# Layers\n\nlayer three holds vectors\n",
    )
    .expect("w");
    let db = Arc::new(
        Database::new(DatabaseConfig {
            db_path: PathBuf::from(":memory:"),
            ..DatabaseConfig::default()
        })
        .await
        .expect("db"),
    );
    let e = SearchEngine::new(db, None, SearchConfig::default())
        .await
        .expect("e");
    e.index_directory(dir.path()).await.expect("index");
    (e, dir)
}

async fn roundtrip(e: &SearchEngine, input: &str) -> Vec<Response> {
    let mut out = Vec::new();
    serve(e, input.as_bytes(), &mut out).await.expect("serve");
    String::from_utf8(out)
        .expect("utf8")
        .lines()
        .map(|l| serde_json::from_str(l).expect("response json"))
        .collect()
}

#[tokio::test]
async fn test_search_returns_hits_with_citations() {
    let (e, _d) = engine_with_doc().await;
    let r = roundtrip(&e, "{\"id\":7,\"method\":\"search\",\"params\":{\"query\":\"vectors\",\"mode\":\"keyword\"}}\n").await;
    assert_eq!(r.len(), 1);
    assert!(r[0].ok, "{:?}", r[0]);
    assert_eq!(r[0].id, json!(7));
    let hits = r[0].result.as_ref().expect("result")["hits"]
        .as_array()
        .expect("hits")
        .clone();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["confidence"], "FACT");
    assert_eq!(hits[0]["citation"]["file_path"], "a.md");
    assert_eq!(hits[0]["citation"]["line_range"], json!([1, 3]));
}

#[tokio::test]
async fn expanded_search_preserves_explain_in_service_response() {
    let (e, dir) = engine_with_doc().await;
    std::fs::write(
        dir.path().join("a.md"),
        "rollback procedure\n\nsee [[b]] for rollback steps\n",
    )
    .expect("write a");
    std::fs::write(dir.path().join("b.md"), "rollback steps\n").expect("write b");
    e.index_directory(dir.path()).await.expect("reindex");

    let responses = roundtrip(
            &e,
            "{\"id\":9,\"method\":\"search\",\"params\":{\"query\":\"rollback\",\"mode\":\"keyword\",\"limit\":1,\"expand_links\":true,\"explain\":true}}\n",
        )
        .await;
    let hits = responses[0].result.as_ref().expect("result")["hits"]
        .as_array()
        .expect("hits");
    assert!(hits.len() >= 2, "direct and linked hits: {hits:?}");
    assert!(
        hits.iter().all(|hit| hit.get("explain").is_some()),
        "{hits:?}"
    );
}

#[tokio::test]
async fn test_errors_do_not_end_session() {
    let (e, _d) = engine_with_doc().await;
    let input = "not json\n\
                     {\"id\":1,\"method\":\"nope\"}\n\
                     {\"id\":2,\"method\":\"search\",\"params\":{}}\n\
                     {\"id\":3,\"method\":\"search\",\"params\":{\"query\":\"x\",\"mode\":\"semantic\"}}\n\
                     {\"id\":4,\"method\":\"search\",\"params\":{\"query\":\"x\",\"limit\":\"ten\"}}\n\
                     {\"id\":5,\"method\":\"index\",\"params\":{\"path\":\"no/such/dir\"}}\n\
                     \n\
                     {\"id\":6,\"method\":\"ping\"}\n";
    let r = roundtrip(&e, input).await;
    let kinds: Vec<_> = r
        .iter()
        .map(|x| x.error.as_ref().map(|b| b.kind.as_str()).unwrap_or("ok"))
        .collect();
    assert_eq!(
        kinds,
        vec![
            "bad_request",
            "bad_request",
            "bad_request",
            "feature_not_enabled",
            "bad_request",
            "bad_request",
            "ok"
        ]
    );
    assert_eq!(r[0].id, Value::Null);
    assert_eq!(r[6].result.as_ref().expect("pong")["pong"], true);
}

#[tokio::test]
async fn test_limits_are_bounded_and_type_checked() {
    let (e, _d) = engine_with_doc().await;
    for limit in [0, 1001] {
        let req = Request {
            id: json!(1),
            method: "search".into(),
            params: json!({"query": "x", "limit": limit}),
        };
        let response = handle_request(&e, req).await;
        assert_eq!(response.error.expect("bad request").kind, "bad_request");
    }
    let req = Request {
        id: json!(2),
        method: "search".into(),
        params: json!({"query": "x", "limit": "100"}),
    };
    assert_eq!(
        handle_request(&e, req)
            .await
            .error
            .expect("bad request")
            .kind,
        "bad_request"
    );
}

#[tokio::test]
async fn test_index_stats_metrics() {
    let (e, d) = engine_with_doc().await;
    let path = d.path().to_string_lossy().replace('\\', "\\\\");
    let input = format!(
        "{{\"id\":1,\"method\":\"index\",\"params\":{{\"path\":\"{path}\"}}}}\n\
             {{\"id\":2,\"method\":\"stats\"}}\n\
             {{\"id\":3,\"method\":\"metrics\"}}\n"
    );
    let r = roundtrip(&e, &input).await;
    assert!(r.iter().all(|x| x.ok), "{r:?}");
    assert_eq!(r[0].result.as_ref().expect("r")["unchanged"], 1);
    assert_eq!(r[1].result.as_ref().expect("r")["documents_total"], 1);
    assert!(r[2].result.as_ref().expect("r")["prometheus"]
        .as_str()
        .expect("s")
        .contains("plaesy_memory_"));
}
