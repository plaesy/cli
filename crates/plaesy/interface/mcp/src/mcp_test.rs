use super::*;
use plaesy_adapter_sqlite::SqliteStore;

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("docs.sqlite")).unwrap();
    (dir, store)
}

fn call(store: &dyn DocumentStore, id: u64, name: &str, arguments: Value) -> Value {
    let request = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    });
    handle(store, &request).expect("permintaan dengan id harus dibalas")
}

fn tool_text(reply: &Value) -> Value {
    let text = reply["result"]["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap()
}

#[test]
fn mcp_initialize_announces_protocol_and_tools_capability() {
    let (_dir, store) = store();
    let reply = handle(
        &store,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
    )
    .unwrap();
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert!(reply["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn mcp_notifications_get_no_reply() {
    let (_dir, store) = store();
    let reply = handle(
        &store,
        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    );
    assert!(reply.is_none());
}

#[test]
fn mcp_tools_list_names_the_four_document_tools() {
    let (_dir, store) = store();
    let reply = handle(
        &store,
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    )
    .unwrap();
    let names: Vec<&str> = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "document_get",
            "document_put",
            "document_list",
            "document_delete"
        ]
    );
}

#[test]
fn mcp_put_get_list_delete_through_tools() {
    let (_dir, store) = store();
    let doc = json!({ "ir": "flow-ir/1", "name": "checkout" });
    let put = call(
        &store,
        3,
        "document_put",
        json!({ "kind": "flow-ir/1", "id": "checkout", "document": doc }),
    );
    assert_eq!(put["result"]["isError"], false);

    let got = call(
        &store,
        4,
        "document_get",
        json!({ "kind": "flow-ir/1", "id": "checkout" }),
    );
    assert_eq!(tool_text(&got), doc);

    let listed = call(&store, 5, "document_list", json!({ "kind": "flow-ir/1" }));
    assert_eq!(tool_text(&listed), json!({ "ids": ["checkout"] }));

    let deleted = call(
        &store,
        6,
        "document_delete",
        json!({ "kind": "flow-ir/1", "id": "checkout" }),
    );
    assert_eq!(tool_text(&deleted), json!({ "deleted": true }));
}

#[test]
fn mcp_storage_errors_are_tool_errors_not_protocol_errors() {
    let (_dir, store) = store();
    let reply = call(&store, 7, "document_get", json!({ "kind": "", "id": "x" }));
    assert!(reply.get("error").is_none(), "bukan galat JSON-RPC");
    assert_eq!(reply["result"]["isError"], true);
    assert!(tool_text(&reply)["error"]
        .as_str()
        .unwrap()
        .contains("kunci tidak valid"));
}

#[test]
fn mcp_unknown_method_and_bad_params_are_json_rpc_errors() {
    let (_dir, store) = store();
    let unknown = handle(
        &store,
        &json!({ "jsonrpc": "2.0", "id": 8, "method": "nope" }),
    )
    .unwrap();
    assert_eq!(unknown["error"]["code"], METHOD_NOT_FOUND);

    let missing_name = handle(
        &store,
        &json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {} }),
    )
    .unwrap();
    assert_eq!(missing_name["error"]["code"], INVALID_PARAMS);

    let missing_arg = call(&store, 10, "document_get", json!({ "kind": "flow-ir/1" }));
    assert_eq!(missing_arg["error"]["code"], INVALID_PARAMS);
}

#[test]
fn mcp_serve_writes_one_json_line_per_reply_and_reports_parse_errors() {
    let (_dir, store) = store();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n",
        "\n",
        "bukan json\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
    );
    let mut output = Vec::new();
    serve(&store, input.as_bytes(), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("setiap baris stdout harus JSON"))
        .collect();
    assert_eq!(
        lines.len(),
        2,
        "tools/list dan galat parse; notifikasi tidak dibalas"
    );
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["error"]["code"], PARSE_ERROR);
}
