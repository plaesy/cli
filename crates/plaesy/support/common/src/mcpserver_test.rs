//! Unit tests for `tests` split out of `mcpserver.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn project_with_memory() -> (tempfile::TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".plaesy/memory")).unwrap();
    fs::write(dir.path().join(".plaesy/memory/notes.md"), "hello").unwrap();
    let server = Server::new(dir.path(), "0.0.1").unwrap();
    (dir, server)
}

#[test]
fn initialize_echoes_a_supported_version_and_falls_back_otherwise() {
    let (_dir, server) = project_with_memory();
    let reply = server
            .handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}))
            .unwrap();
    assert_eq!(reply["result"]["protocolVersion"], "2024-11-05");
    let reply = server
            .handle(&json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}))
            .unwrap();
    assert_eq!(reply["result"]["protocolVersion"], SUPPORTED_VERSIONS[0]);
}

#[test]
fn notifications_get_no_reply() {
    let (_dir, server) = project_with_memory();
    assert!(server
        .handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .is_none());
}

#[test]
fn list_and_read_stay_inside_plaesy() {
    let (_dir, server) = project_with_memory();
    assert!(server.list("").unwrap().contains("memory/notes.md\t5"));
    assert_eq!(server.read("memory/notes.md").unwrap(), "hello");
    assert!(server.read("../secret.md").is_err());
    assert!(server.read("/etc/passwd").is_err());
    assert!(server.read("image.png").is_err());
}

#[test]
fn write_creates_folders_and_refuses_analysis() {
    let (dir, server) = project_with_memory();
    server
        .write("memory/new/overview.md", "# Overview")
        .unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join(".plaesy/memory/new/overview.md")).unwrap(),
        "# Overview"
    );
    assert!(server.write("analysis/x.json", "{}").is_err());
    assert!(server.write("../outside.md", "x").is_err());
}

#[test]
fn tool_failure_is_a_result_not_a_protocol_error() {
    let (_dir, server) = project_with_memory();
    let reply = server
            .handle(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"plaesy_read","arguments":{"path":"missing.md"}}}))
            .unwrap();
    assert_eq!(reply["result"]["isError"], true);
    let bad = server
            .handle(&json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope","arguments":{}}}))
            .unwrap();
    assert_eq!(bad["error"]["code"], -32602);
}

/// Sends one request with an id and returns the reply.
fn rpc(server: &Server, id: i64, method: &str, params: Value) -> Value {
    server
        .handle(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
        .unwrap()
}

#[test]
fn tools_list_names_the_three_plaesy_tools() {
    let (_dir, server) = project_with_memory();
    let reply = rpc(&server, 1, "tools/list", Value::Null);
    let names: Vec<&str> = reply["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["plaesy_list", "plaesy_read", "plaesy_write"]);
}

#[test]
fn initialize_advertises_tools_and_resources() {
    let (_dir, server) = project_with_memory();
    let reply = rpc(&server, 1, "initialize", json!({}));
    assert!(reply["result"]["capabilities"]["tools"].is_object());
    assert!(reply["result"]["capabilities"]["resources"].is_object());
    assert_eq!(reply["result"]["protocolVersion"], SUPPORTED_VERSIONS[0]);
}

#[test]
fn tool_calls_missing_arguments_are_invalid_params() {
    let (_dir, server) = project_with_memory();
    let no_name = rpc(&server, 1, "tools/call", json!({}));
    assert_eq!(no_name["error"]["code"], -32602);
    let no_path = rpc(
        &server,
        2,
        "tools/call",
        json!({"name":"plaesy_read","arguments":{}}),
    );
    assert_eq!(no_path["error"]["code"], -32602);
    let no_content = rpc(
        &server,
        3,
        "tools/call",
        json!({"name":"plaesy_write","arguments":{"path":"memory/a.md"}}),
    );
    assert_eq!(no_content["error"]["code"], -32602);
}

#[test]
fn write_through_tools_call_is_listed_and_read_back() {
    let (dir, server) = project_with_memory();
    let wrote = rpc(
        &server,
        1,
        "tools/call",
        json!({"name":"plaesy_write","arguments":{"path":"memory/new.md","content":"# New"}}),
    );
    assert!(wrote["result"]["isError"].is_null());
    assert_eq!(
        fs::read_to_string(dir.path().join(".plaesy/memory/new.md")).unwrap(),
        "# New"
    );
    let listed = rpc(
        &server,
        2,
        "tools/call",
        json!({"name":"plaesy_list","arguments":{"path":"memory"}}),
    );
    let text = listed["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("memory/new.md"));
}

#[test]
fn resources_list_offers_only_readable_files_with_uri_and_mime_type() {
    let (dir, server) = project_with_memory();
    fs::write(dir.path().join(".plaesy/memory/picture.png"), "png").unwrap();
    let reply = rpc(&server, 1, "resources/list", Value::Null);
    let resources = reply["result"]["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["uri"], "plaesy:///memory/notes.md");
    assert_eq!(resources[0]["name"], "memory/notes.md");
    assert_eq!(resources[0]["mimeType"], "text/markdown");
    assert_eq!(resources[0]["size"], 5);
}

#[test]
fn resources_read_returns_the_text_of_a_listed_uri() {
    let (_dir, server) = project_with_memory();
    let reply = rpc(
        &server,
        1,
        "resources/read",
        json!({"uri":"plaesy:///memory/notes.md"}),
    );
    let content = &reply["result"]["contents"][0];
    assert_eq!(content["uri"], "plaesy:///memory/notes.md");
    assert_eq!(content["mimeType"], "text/markdown");
    assert_eq!(content["text"], "hello");
}

#[test]
fn resources_read_reports_unknown_and_escaping_uris_as_not_found() {
    let (dir, server) = project_with_memory();
    fs::write(dir.path().join(".plaesy/memory/picture.png"), "png").unwrap();
    let uris = [
        "plaesy:///memory/missing.md",
        "plaesy:///../secret.md",
        "plaesy:///memory/picture.png",
        "plaesy:///",
        "file:///etc/passwd",
    ];
    for (id, uri) in (10..).zip(uris) {
        let reply = rpc(&server, id, "resources/read", json!({"uri": uri}));
        assert_eq!(reply["error"]["code"], -32002, "{uri}");
        assert!(reply["error"]["message"].as_str().unwrap().contains(uri));
    }
    let no_uri = rpc(&server, 20, "resources/read", json!({}));
    assert_eq!(no_uri["error"]["code"], -32602);
}

#[test]
fn resources_list_is_empty_without_a_plaesy_folder() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::new(dir.path(), "0.0.1").unwrap();
    let reply = rpc(&server, 1, "resources/list", Value::Null);
    assert_eq!(reply["result"]["resources"], json!([]));
    let read = rpc(
        &server,
        2,
        "resources/read",
        json!({"uri":"plaesy:///notes.md"}),
    );
    assert_eq!(read["error"]["code"], -32002);
}

#[test]
fn resource_templates_list_is_empty() {
    let (_dir, server) = project_with_memory();
    let reply = rpc(&server, 1, "resources/templates/list", Value::Null);
    assert_eq!(reply["result"]["resourceTemplates"], json!([]));
}

#[test]
fn malformed_messages_get_invalid_request_errors() {
    let (_dir, server) = project_with_memory();
    let not_object = server.handle(&json!([1, 2])).unwrap();
    assert_eq!(not_object["error"]["code"], -32600);
    assert_eq!(not_object["id"], Value::Null);
    let no_method = server.handle(&json!({"jsonrpc":"2.0","id":11})).unwrap();
    assert_eq!(no_method["error"]["code"], -32600);
    assert_eq!(no_method["id"], 11);
    let bad_method = server
        .handle(&json!({"jsonrpc":"2.0","id":12,"method":5}))
        .unwrap();
    assert_eq!(bad_method["error"]["code"], -32600);
    assert_eq!(bad_method["id"], 12);
}

#[test]
fn client_replies_get_no_reply_and_unknown_methods_echo_the_id() {
    let (_dir, server) = project_with_memory();
    assert!(server
        .handle(&json!({"jsonrpc":"2.0","id":13,"result":{}}))
        .is_none());
    let reply = rpc(&server, 14, "nope/x", Value::Null);
    assert_eq!(reply["id"], 14);
    assert_eq!(reply["error"]["code"], -32601);
    assert_eq!(reply["error"]["message"], "method not found: nope/x");
}

#[test]
fn serve_answers_each_request_and_skips_blank_lines_and_notifications() {
    let (_dir, server) = project_with_memory();
    let input = "garbage\n\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
    let mut output = Vec::new();
    server.serve(input.as_bytes(), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["error"]["code"], -32700);
    assert_eq!(lines[0]["id"], Value::Null);
    assert_eq!(lines[1]["id"], 1);
    assert_eq!(lines[1]["result"], json!({}));
}
