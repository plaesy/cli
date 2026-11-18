//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::memory::config::SearchConfig;
use crate::memory::storage::{Database, DatabaseConfig};
use std::path::PathBuf;
use std::sync::Arc;

async fn engine() -> (SearchEngine, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(
            dir.path().join("a.md"),
            "---\ntier: semantic\nconfidence: high\nlast_verified: 2099-01-01\n---\n# Cats\n\nCats purr when content. See [[b]].\n",
        )
        .expect("w");
    std::fs::write(dir.path().join("b.md"), "# Dogs\n\nDogs bark.\n").expect("w");
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
        .expect("engine");
    e.index_directory(dir.path()).await.expect("index");
    (e, dir)
}

async fn rpc(e: &SearchEngine, msg: Value) -> Value {
    handle_message(e, &msg).await.expect("reply")
}

#[tokio::test]
async fn handshake_and_tool_listing() {
    let (e, _d) = engine().await;
    let init = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}),
        )
        .await;
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "plaesy-memory");
    let future = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2031-01-01"}}),
        )
        .await;
    assert_eq!(
        future["result"]["protocolVersion"], LATEST,
        "unknown version -> ours"
    );

    assert!(handle_message(
        &e,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .await
    .is_none());
    let list = rpc(&e, json!({"jsonrpc":"2.0","id":3,"method":"tools/list"})).await;
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "memory_search",
            "memory_backlinks",
            "memory_hygiene",
            "memory_missing",
            "memory_curate",
            "memory_remember",
            "memory_resolve_gap",
            "memory_stats"
        ]
    );
    let bad = rpc(
        &e,
        json!({"jsonrpc":"2.0","id":4,"method":"resources/list"}),
    )
    .await;
    assert_eq!(bad["error"]["code"], -32601);
}

#[tokio::test]
async fn search_tool_returns_cited_text_and_structured_hits() {
    let (e, _d) = engine().await;
    let r = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"memory_search","arguments":{"query":"purr","mode":"keyword","tier":"semantic","min_trust":"high"}}}),
        )
        .await;
    let res = &r["result"];
    assert_eq!(res["isError"], false);
    let text = res["content"][0]["text"].as_str().expect("text");
    assert!(
        text.contains("a.md:") && text.contains("[FACT]") && text.contains("tier semantic"),
        "{text}"
    );
    assert!(text.contains("> Cats purr when content"), "{text}");
    assert_eq!(
        res["structuredContent"]["hits"][0]["citation"]["file_path"],
        "a.md"
    );

    let none = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_search","arguments":{"query":"bark","tier":"semantic"}}}),
        )
        .await;
    assert_eq!(none["result"]["content"][0]["text"], "No matching notes.");
}

#[tokio::test]
async fn mcp_explain_includes_breakdown_when_requested() {
    let (e, _d) = engine().await;
    let r = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"memory_search","arguments":{"query":"purr","explain":true}}}),
        )
        .await;
    let res = &r["result"];
    assert_eq!(res["isError"], false);
    let hits = res["structuredContent"]["hits"].as_array().expect("hits");
    assert!(!hits.is_empty());
    let ex = &hits[0]["explain"];
    assert!(ex.is_object(), "explain must be present when requested");
    assert!(ex["fused"].is_number(), "fused leg must be present");
    assert!(ex["fts"].is_number(), "fts leg must be present");
}

/// The outputSchema promises fields; every required field must be present in a real result.
#[tokio::test]
async fn mcp_search_result_conforms_to_output_schema() {
    let (e, _d) = engine().await;
    let listed = rpc(&e, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})).await;
    let tool = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .find(|t| t["name"] == "memory_search")
        .expect("memory_search listed")
        .clone();
    let schema = &tool["outputSchema"]["properties"]["hits"]["items"];
    let r = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_search","arguments":{"query":"purr"}}}),
        )
        .await;
    let hits = r["result"]["structuredContent"]["hits"]
        .as_array()
        .expect("hits");
    assert!(!hits.is_empty());
    for field in schema["required"].as_array().expect("required") {
        let key = field.as_str().expect("key");
        assert!(
            hits[0].get(key).is_some(),
            "hit is missing required field {key}"
        );
    }
    let cit_required = schema["properties"]["citation"]["required"]
        .as_array()
        .expect("citation required");
    for field in cit_required {
        let key = field.as_str().expect("key");
        assert!(
            hits[0]["citation"].get(key).is_some(),
            "citation is missing {key}"
        );
    }
}

#[tokio::test]
async fn mcp_explain_omitted_by_default() {
    let (e, _d) = engine().await;
    let r = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"memory_search","arguments":{"query":"purr"}}}),
        )
        .await;
    let res = &r["result"];
    assert_eq!(res["isError"], false);
    let hits = res["structuredContent"]["hits"].as_array().expect("hits");
    if let Some(h) = hits.first() {
        assert!(
            h.get("explain").is_none(),
            "explain must be absent by default"
        );
    }
}

#[tokio::test]
async fn tool_errors_are_results_not_protocol_errors() {
    let (e, _d) = engine().await;
    for args in [
        json!({"name":"memory_search","arguments":{}}),
        json!({"name":"memory_search","arguments":{"query":"x","mode":"fuzzy"}}),
        json!({"name":"memory_search","arguments":{"query":"x","tier":"bogus"}}),
        json!({"name":"nope","arguments":{}}),
    ] {
        let r = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":args}),
        )
        .await;
        assert_eq!(r["result"]["isError"], true, "{r}");
        assert!(r.get("error").is_none());
    }
}

#[tokio::test]
async fn backlinks_hygiene_and_stats_tools() {
    let (e, _d) = engine().await;
    let b = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"memory_backlinks","arguments":{"path":"b"}}}),
        )
        .await;
    assert!(b["result"]["content"][0]["text"]
        .as_str()
        .expect("t")
        .starts_with("a.md:"));
    let h = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_hygiene","arguments":{}}}),
        )
        .await;
    assert_eq!(h["result"]["structuredContent"]["checked"], 2);
    let s = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"memory_stats","arguments":{}}}),
        )
        .await;
    assert_eq!(s["result"]["structuredContent"]["documents_total"], 2);
}

#[tokio::test]
async fn missing_tool_lists_gaps_read_only() {
    let (e, _d) = engine().await;
    e.database()
        .record_query("unanswered question", None)
        .expect("record");
    let m = rpc(
            &e,
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"memory_missing","arguments":{"limit":5}}}),
        )
        .await;
    assert_eq!(
        m["result"]["structuredContent"]["query_gaps"][0]["query"],
        "unanswered question"
    );
    // Read-only: calling the tool must not record a query of its own.
    let before = e
        .database()
        .open_query_gaps(10, chrono::Utc::now())
        .expect("gaps")
        .len();
    rpc(
            &e,
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"memory_missing","arguments":{}}}),
        )
        .await;
    let after = e
        .database()
        .open_query_gaps(10, chrono::Utc::now())
        .expect("gaps")
        .len();
    assert_eq!(before, after);
}

#[tokio::test]
async fn remember_creates_indexed_note_and_never_overwrites() {
    let (e, dir) = engine().await;
    e.set_watch_root(dir.path().to_path_buf());
    let call = |id: u64, args: Value| json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"memory_remember","arguments":args}});
    let ok = rpc(
            &e,
            call(6, json!({"title":"Rollback plan","body":"Revert the release tag.","folder":"ops","tier":"procedural"})),
        )
        .await;
    assert_eq!(ok["result"]["isError"], false);
    let file = dir.path().join("ops").join("rollback-plan.md");
    let text = std::fs::read_to_string(&file).expect("note written");
    assert!(text.contains("source: agent"));
    // Indexed at once: the new note is findable.
    let hits = e.search("revert release tag", 3).await.expect("search");
    assert!(hits
        .iter()
        .any(|h| h.source_path.ends_with("rollback-plan.md")));
    // Second write to the same name is refused and the file is unchanged.
    let dup = rpc(
        &e,
        call(
            7,
            json!({"title":"Rollback plan","body":"Other text.","folder":"ops"}),
        ),
    )
    .await;
    assert_eq!(dup["result"]["isError"], true);
    assert_eq!(std::fs::read_to_string(&file).expect("still there"), text);
    // Folders may not leave the notes root.
    let escape = rpc(
        &e,
        call(8, json!({"title":"x","body":"y","folder":"../outside"})),
    )
    .await;
    assert_eq!(escape["result"]["isError"], true);
    assert!(!dir.path().parent().unwrap().join("outside").exists());
}

#[tokio::test]
async fn serve_loop_survives_garbage_lines() {
    let (e, _d) = engine().await;
    let input = b"not json\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n";
    let mut out: Vec<u8> = Vec::new();
    serve(&e, &input[..], &mut out).await.expect("serve");
    let lines: Vec<Value> = String::from_utf8(out)
        .expect("utf8")
        .lines()
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["error"]["code"], -32700);
    assert_eq!(lines[1]["id"], 7);
}
