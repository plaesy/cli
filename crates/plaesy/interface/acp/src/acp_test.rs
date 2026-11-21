use super::*;
use plaesy_adapter_sqlite::SqliteStore;

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("docs.sqlite")).unwrap();
    (dir, store)
}

/// Balasan terakhir (request) dari daftar pesan yang dikembalikan `handle`.
fn reply(messages: &[Value]) -> &Value {
    messages.last().expect("harus ada balasan")
}

fn request(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
}

#[test]
fn acp_initialize_agrees_on_protocol_version_one() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let messages = agent.handle(&request("initialize", json!({ "protocolVersion": 1 })));
    let result = &reply(&messages)["result"];
    assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(result["agentInfo"]["name"], "plaesy");
    assert!(result["agentCapabilities"]["promptCapabilities"].is_object());
}

#[test]
fn acp_session_new_requires_absolute_cwd_and_returns_session_id() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let relative = agent.handle(&request(
        "session/new",
        json!({ "cwd": "relatif", "mcpServers": [] }),
    ));
    assert_eq!(reply(&relative)["error"]["code"], INVALID_PARAMS);

    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let created = agent.handle(&request(
        "session/new",
        json!({ "cwd": cwd, "mcpServers": [] }),
    ));
    assert_eq!(reply(&created)["result"]["sessionId"], "sess-1");
}

#[test]
fn acp_prompt_sends_session_update_then_end_turn() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    agent.handle(&request(
        "session/new",
        json!({ "cwd": cwd, "mcpServers": [] }),
    ));

    let messages = agent.handle(&request(
        "session/prompt",
        json!({ "sessionId": "sess-1", "prompt": [{ "type": "text", "text": "halo" }] }),
    ));
    assert_eq!(messages.len(), 2, "satu session/update lalu satu balasan");
    assert_eq!(messages[0]["method"], "session/update");
    assert_eq!(
        messages[0]["params"]["update"]["sessionUpdate"],
        "agent_message_chunk"
    );
    assert_eq!(reply(&messages)["result"]["stopReason"], "end_turn");
}

#[test]
fn acp_prompt_for_unknown_session_is_invalid_params() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let messages = agent.handle(&request(
        "session/prompt",
        json!({ "sessionId": "tidak-ada", "prompt": [] }),
    ));
    assert_eq!(reply(&messages)["error"]["code"], INVALID_PARAMS);
}

#[test]
fn acp_cancel_notification_gets_no_reply() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let messages = agent.handle(&json!({
        "jsonrpc": "2.0",
        "method": "session/cancel",
        "params": { "sessionId": "sess-1" }
    }));
    assert!(messages.is_empty());
}

#[test]
fn acp_plaesy_document_extension_roundtrip() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let put = agent.handle(&request(
        "_plaesy/document/put",
        json!({ "kind": "flow-ir/1", "id": "a", "document": {"n": 1} }),
    ));
    assert_eq!(reply(&put)["result"]["ok"], true);

    let got = agent.handle(&request(
        "_plaesy/document/get",
        json!({ "kind": "flow-ir/1", "id": "a" }),
    ));
    assert_eq!(reply(&got)["result"], json!({"n": 1}));

    let listed = agent.handle(&request(
        "_plaesy/document/list",
        json!({ "kind": "flow-ir/1" }),
    ));
    assert_eq!(reply(&listed)["result"]["ids"], json!(["a"]));

    let deleted = agent.handle(&request(
        "_plaesy/document/delete",
        json!({ "kind": "flow-ir/1", "id": "a" }),
    ));
    assert_eq!(reply(&deleted)["result"]["deleted"], true);
}

#[test]
fn acp_invalid_key_is_invalid_params_not_server_error() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let messages = agent.handle(&request("_plaesy/document/list", json!({ "kind": "" })));
    assert_eq!(reply(&messages)["error"]["code"], INVALID_PARAMS);
}

#[test]
fn acp_unknown_method_is_method_not_found() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let messages = agent.handle(&request("nope", json!({})));
    assert_eq!(reply(&messages)["error"]["code"], METHOD_NOT_FOUND);
}

#[test]
fn acp_serve_writes_only_json_lines_and_reports_parse_errors() {
    let (_dir, store) = store();
    let agent = Agent::new(&store);
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1}}\n",
        "bukan json\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"session/cancel\",\"params\":{}}\n",
    );
    let mut output = Vec::new();
    serve(&agent, input.as_bytes(), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("setiap baris stdout harus JSON"))
        .collect();
    assert_eq!(
        lines.len(),
        2,
        "balasan initialize dan galat parse; cancel tidak dibalas"
    );
    assert_eq!(lines[0]["id"], 1);
    assert_eq!(lines[1]["error"]["code"], PARSE_ERROR);
}

struct Echo;

impl ModelBackend for Echo {
    fn complete(&self, prompt: &str) -> Result<String, String> {
        Ok(format!("gema: {prompt}"))
    }
}

struct Broken;

impl ModelBackend for Broken {
    fn complete(&self, _prompt: &str) -> Result<String, String> {
        Err("kunci API tidak ada".to_owned())
    }
}

fn session(agent: &Agent<'_>) {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.keep().to_string_lossy().to_string();
    agent.handle(&request(
        "session/new",
        json!({ "cwd": cwd, "mcpServers": [] }),
    ));
}

#[test]
fn acp_prompt_text_goes_to_the_configured_model() {
    let (_dir, store) = store();
    let agent = Agent::with_model(&store, Box::new(Echo));
    session(&agent);
    let messages = agent.handle(&request(
        "session/prompt",
        json!({
            "sessionId": "sess-1",
            "prompt": [
                { "type": "text", "text": "satu" },
                { "type": "image", "data": "abc" },
                { "type": "text", "text": "dua" }
            ]
        }),
    ));
    assert_eq!(
        messages[0]["params"]["update"]["content"]["text"],
        "gema: satu\ndua"
    );
    assert_eq!(reply(&messages)["result"]["stopReason"], "end_turn");
}

#[test]
fn acp_model_failure_is_a_server_error_not_a_fake_answer() {
    let (_dir, store) = store();
    let agent = Agent::with_model(&store, Box::new(Broken));
    session(&agent);
    let messages = agent.handle(&request(
        "session/prompt",
        json!({ "sessionId": "sess-1", "prompt": [{ "type": "text", "text": "halo" }] }),
    ));
    assert_eq!(reply(&messages)["error"]["code"], SERVER_ERROR);
    assert!(reply(&messages)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("kunci API tidak ada"));
    assert_eq!(messages.len(), 1, "tidak ada update palsu saat model gagal");
}
