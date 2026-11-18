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
