use super::*;
use std::io::Cursor;
use std::path::PathBuf;

fn framed(message: &Value) -> Vec<u8> {
    encode(message)
}

#[test]
fn framing_round_trips_one_message() {
    let message = json!({"jsonrpc": "2.0", "id": 7, "result": {"ok": true}});
    let mut reader = Cursor::new(framed(&message));
    assert_eq!(read_message(&mut reader).expect("read"), message);
}

#[test]
fn utf16_column_counts_surrogate_pairs_as_two_units() {
    assert_eq!(utf16_column("abc", 1), 0);
    assert_eq!(utf16_column("abc", 3), 2);
    assert_eq!(utf16_column("😀x", 2), 2, "an emoji is two UTF-16 units");
    assert_eq!(utf16_column("añb", 3), 2);
}

#[test]
fn targets_are_checked_before_anything_runs() {
    assert_eq!(
        parse_target("src/a.rs:5:3").expect("valid"),
        ("src/a.rs".to_owned(), 5, 3)
    );
    assert!(parse_target("../rahasia.rs:1:1").is_err());
    assert!(parse_target("/abs/a.rs:1:1").is_err());
    assert!(parse_target("src/a.rs:0:1").is_err());
    assert!(parse_target("src/a.rs").is_err());
}

#[test]
fn servers_parse_and_repository_servers_need_approval() {
    let value = json!({"lsp": {"servers": [
        {"name": "rust", "extensions": [".RS"], "command": "rust-analyzer", "timeout": "5s"}
    ]}});
    let user = parse_servers(&value, Scope::User).expect("valid");
    assert_eq!(user[0].extensions, vec!["rs".to_owned()]);
    assert!(is_trusted(&user[0], &[]));

    let repo = parse_servers(&value, Scope::Repo).expect("valid");
    assert!(!is_trusted(&repo[0], &[]));
    assert!(is_trusted(&repo[0], &[fingerprint(&repo[0])]));

    let empty = json!({"lsp": {"servers": [{"name": " ", "extensions": [], "command": "x"}]}});
    assert!(parse_servers(&empty, Scope::User).is_err());
}

#[test]
fn definition_locations_are_shown_relative_to_the_repository() {
    let repo = Path::new("C:/repo");
    let result = json!([{
        "uri": "file:///C:/repo/src/a.rs",
        "range": {"start": {"line": 4, "character": 2}, "end": {"line": 4, "character": 6}}
    }]);
    assert_eq!(format_result("definition", &result, repo), "src/a.rs:5:3");
    assert_eq!(
        format_result("references", &Value::Null, repo),
        "no results"
    );
}

#[test]
fn hover_text_reads_markup_and_plain_strings() {
    let repo = Path::new("C:/repo");
    let markup = json!({"contents": {"kind": "markdown", "value": "fn main()"}});
    assert_eq!(format_result("hover", &markup, repo), "fn main()");
    let plain = json!({"contents": ["satu", {"value": "dua"}]});
    assert_eq!(format_result("hover", &plain, repo), "satu\ndua");
    assert_eq!(
        format_result("hover", &json!({}), repo),
        "no hover information"
    );
}

#[test]
fn client_asks_for_a_definition_over_framed_messages() {
    // The server answers initialize (id 1) and the definition (id 2); notifications get no reply.
    let mut script = Vec::new();
    script.extend(framed(
        &json!({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}}),
    ));
    script.extend(framed(&json!({"jsonrpc": "2.0", "id": 2, "result": [{
        "uri": "file:///C:/repo/src/b.rs",
        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}
    }]})));
    let mut written = Vec::new();
    let mut client = Client::new(Cursor::new(script), &mut written);
    client.initialize("file:///C:/repo").expect("initialize");
    let result = client
        .request_at("textDocument/definition", "file:///C:/repo/src/a.rs", 3, 4)
        .expect("definition");
    assert_eq!(
        format_result("definition", &result, Path::new("C:/repo")),
        "src/b.rs:1:1"
    );
    let sent = String::from_utf8(written).expect("utf8");
    assert!(sent.contains("\"method\":\"initialized\""));
    assert!(sent.contains("\"method\":\"textDocument/definition\""));
}

#[test]
fn uri_to_path_undoes_the_encoding_of_spaces_and_colons() {
    assert_eq!(uri_to_path("file:///C:/my%20repo/a.rs"), "C:/my repo/a.rs");
    assert_eq!(
        file_uri(&PathBuf::from("C:/my repo")),
        "file:///C:/my%20repo"
    );
}
