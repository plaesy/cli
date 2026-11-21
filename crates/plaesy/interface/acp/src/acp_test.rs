use super::*;
use plaesy_adapter_sqlite::SqliteStore;

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("docs.sqlite")).unwrap();
    (dir, store)
}

fn rpc(store: &dyn DocumentStore, method: &str, params: Value) -> Value {
    handle(
        store,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }),
    )
    .expect("permintaan dengan id harus dibalas")
}

#[test]
fn acp_initialize_reports_draft_protocol_version() {
    let (_dir, store) = store();
    let reply = rpc(&store, "initialize", json!({}));
    assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSION);
}

#[test]
fn acp_document_methods_roundtrip() {
    let (_dir, store) = store();
    let put = rpc(
        &store,
        "document/put",
        json!({ "kind": "flow-ir/1", "id": "a", "document": {"n": 1} }),
    );
    assert_eq!(put["result"]["ok"], true);

    let got = rpc(
        &store,
        "document/get",
        json!({ "kind": "flow-ir/1", "id": "a" }),
    );
    assert_eq!(got["result"], json!({"n": 1}));

    let listed = rpc(&store, "document/list", json!({ "kind": "flow-ir/1" }));
    assert_eq!(listed["result"]["ids"], json!(["a"]));

    let deleted = rpc(
        &store,
        "document/delete",
        json!({ "kind": "flow-ir/1", "id": "a" }),
    );
    assert_eq!(deleted["result"]["deleted"], true);
}

#[test]
fn acp_missing_parameters_are_invalid_params() {
    let (_dir, store) = store();
    let reply = rpc(&store, "document/get", json!({ "kind": "flow-ir/1" }));
    assert_eq!(reply["error"]["code"], INVALID_PARAMS);

    let no_document = rpc(
        &store,
        "document/put",
        json!({ "kind": "flow-ir/1", "id": "a" }),
    );
    assert_eq!(no_document["error"]["code"], INVALID_PARAMS);
}

#[test]
fn acp_invalid_key_is_invalid_params_not_server_error() {
    let (_dir, store) = store();
    let reply = rpc(&store, "document/list", json!({ "kind": "" }));
    assert_eq!(reply["error"]["code"], INVALID_PARAMS);
}

#[test]
fn acp_unknown_method_is_method_not_found() {
    let (_dir, store) = store();
    let reply = rpc(&store, "nope", json!({}));
    assert_eq!(reply["error"]["code"], METHOD_NOT_FOUND);
}

#[test]
fn acp_serve_outputs_one_json_line_per_request() {
    let (_dir, store) = store();
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n",
        "bukan json\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"initialize\"}\n",
    );
    let mut output = Vec::new();
    serve(&store, input.as_bytes(), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("setiap baris stdout harus JSON"))
        .collect();
    assert_eq!(lines.len(), 2, "notifikasi tidak dibalas");
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["error"]["code"], PARSE_ERROR);
}
