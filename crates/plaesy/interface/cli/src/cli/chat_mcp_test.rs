use super::*;
use std::io::Cursor;

fn server(scope: Scope, allow: &[&str]) -> McpServer {
    McpServer {
        name: "docs".to_owned(),
        command: "docs-mcp".to_owned(),
        args: vec!["--read-only".to_owned()],
        enabled: true,
        allow_tools: allow.iter().map(|a| (*a).to_owned()).collect(),
        timeout: Duration::from_secs(5),
        scope,
    }
}

fn canned(lines: &[&str]) -> Client<Cursor<Vec<u8>>, Vec<u8>> {
    let mut bytes = Vec::new();
    for line in lines {
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    Client::new(Cursor::new(bytes), Vec::new())
}

const INIT_OK: &str =
    r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;

#[test]
fn parse_reads_defaults_and_rejects_bad_values() {
    let value = serde_json::json!({"mcp": {"servers": [{"name": "docs", "command": "docs-mcp"}]}});
    let parsed = parse_servers(&value, Scope::User).expect("valid");
    assert_eq!(parsed[0].timeout, Duration::from_secs(10));
    assert!(parsed[0].enabled);
    assert!(
        parsed[0].allow_tools.is_empty(),
        "no allow_tools means no tools"
    );

    let bad_timeout =
        serde_json::json!({"mcp": {"servers": [{"name": "d", "command": "c", "timeout": "soon"}]}});
    assert!(parse_servers(&bad_timeout, Scope::User).is_err());

    let empty_name = serde_json::json!({"mcp": {"servers": [{"name": " ", "command": "c"}]}});
    assert!(parse_servers(&empty_name, Scope::User).is_err());

    assert!(parse_servers(&serde_json::Value::Null, Scope::User)
        .expect("ok")
        .is_empty());
}

#[test]
fn fingerprint_changes_with_the_command_line_and_repo_servers_need_approval() {
    let base = server(Scope::Repo, &["search"]);
    let mut changed = base.clone();
    changed.args.push("--write".to_owned());
    assert_ne!(fingerprint(&base), fingerprint(&changed));

    assert!(!is_trusted(&base, &[]));
    assert!(is_trusted(&base, &[fingerprint(&base)]));
    assert!(is_trusted(&server(Scope::User, &[]), &[]));
}

#[test]
fn only_allowed_tools_survive() {
    let tools = vec![
        Tool {
            name: "search".to_owned(),
            description: String::new(),
        },
        Tool {
            name: "delete".to_owned(),
            description: String::new(),
        },
    ];
    let kept = allowed(&server(Scope::User, &["search"]), tools);
    assert_eq!(
        kept.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        vec!["search"]
    );
}

#[test]
fn client_lists_and_calls_tools_over_jsonrpc() {
    let tools = r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"search","description":"cari"},{"name":"delete"}]}}"#;
    let call = r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"hasil"}]}}"#;
    let mut client = canned(&[INIT_OK, tools, call]);

    client.initialize().expect("initialize");
    let listed = client.list_tools().expect("tools/list");
    assert_eq!(listed[0].name, "search");
    assert_eq!(listed[0].description, "cari");
    assert_eq!(listed[1].description, "", "a missing description is empty");
    assert_eq!(
        client
            .call_tool("search", serde_json::json!({"q": "x"}))
            .expect("call"),
        "hasil"
    );

    let sent = String::from_utf8(client.writer.clone()).expect("utf8");
    assert!(sent.contains("notifications/initialized"));
    assert!(sent.contains("tools/call"));
}

#[test]
fn replies_for_other_ids_and_notifications_are_skipped() {
    let notice = r#"{"jsonrpc":"2.0","method":"notifications/message","params":{}}"#;
    let stale = r#"{"jsonrpc":"2.0","id":99,"result":{}}"#;
    let mut client = canned(&[notice, stale, INIT_OK]);
    assert!(client.initialize().is_ok());
}

#[test]
fn tool_error_and_closed_connection_are_reported() {
    let err = r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"gagal"}],"isError":true}}"#;
    let mut client = canned(&[INIT_OK, err]);
    client.initialize().expect("init");
    assert_eq!(
        client.call_tool("x", serde_json::json!({})),
        Err("gagal".to_owned())
    );

    let mut closed = canned(&[]);
    assert!(closed.initialize().is_err());
}

#[test]
fn call_refuses_a_tool_outside_the_allow_list_before_starting_anything() {
    let servers = vec![server(Scope::User, &["search"])];
    let err = call(&servers, &[], "docs", "delete", serde_json::json!({})).expect_err("refused");
    assert!(err.contains("not allowed"), "error was: {err}");
    assert!(call(&servers, &[], "nope", "search", serde_json::json!({})).is_err());
}

#[test]
fn list_marks_unapproved_repo_servers_without_starting_them() {
    let servers = vec![server(Scope::Repo, &["search"])];
    let text = list_all(&servers, &[]);
    assert!(text.contains("not approved"), "output was: {text}");
    assert!(text.contains(&fingerprint(&servers[0])));
}

#[test]
fn a_missing_command_is_a_clear_error() {
    let mut broken = server(Scope::User, &[]);
    broken.command = "definitely-not-a-real-mcp-binary".to_owned();
    let err = with_server(&broken, |_client| Ok(())).expect_err("no such command");
    assert!(err.contains("docs"), "error was: {err}");
}
