//! Unit tests for `remote.rs`. They exercise routing, authentication, parsing, and response bodies
//! without a socket. Included via `#[path]`.

use super::*;
use crate::tasks::Status;
use std::cell::Cell;
use std::fs;
use std::net::Ipv4Addr;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn request(method: &str, path: &str, authorization: Option<&str>) -> Request {
    Request {
        method: method.to_owned(),
        path: path.to_owned(),
        authorization: authorization.map(str::to_owned),
    }
}

fn task(status: Status, name: &str) -> TaskRef {
    TaskRef {
        spec: "plaesy-agent-platform".to_owned(),
        status,
        name: name.to_owned(),
        title: format!("Title of {name}"),
        phase: "1".to_owned(),
    }
}

#[test]
fn loopback_runs_without_token_but_other_addresses_need_one() {
    assert!(validate_config(IpAddr::V4(Ipv4Addr::LOCALHOST), None).is_ok());
    assert!(validate_config(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), None).is_ok());
    assert!(matches!(
        validate_config(IpAddr::V4(Ipv4Addr::UNSPECIFIED), None),
        Err(RemoteError::TokenRequired(_))
    ));
    assert!(matches!(
        validate_config(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5)), None),
        Err(RemoteError::TokenRequired(_))
    ));
    assert!(validate_config(IpAddr::V4(Ipv4Addr::UNSPECIFIED), Some(TOKEN)).is_ok());
}

#[test]
fn token_must_be_long_and_unpadded() {
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert!(matches!(
        validate_config(loopback, Some("short")),
        Err(RemoteError::WeakToken)
    ));
    let padded = format!(" {TOKEN}");
    assert!(matches!(
        validate_config(loopback, Some(&padded)),
        Err(RemoteError::WeakToken)
    ));
    assert!(matches!(
        validate_config(loopback, Some("")),
        Err(RemoteError::WeakToken)
    ));
}

#[test]
fn auth_passes_everything_without_a_configured_token() {
    assert!(check_auth(None, None));
    assert!(check_auth(None, Some("Bearer anything")));
}

#[test]
fn auth_requires_matching_bearer_token_when_configured() {
    assert!(!check_auth(Some(TOKEN), None));
    assert!(!check_auth(Some(TOKEN), Some("Basic dXNlcjpwYXNz")));
    assert!(!check_auth(Some(TOKEN), Some("Bearer ")));
    assert!(!check_auth(Some(TOKEN), Some("Bearer wrong-token")));
    assert!(!check_auth(Some(TOKEN), Some(TOKEN)));
    assert!(check_auth(Some(TOKEN), Some(&format!("Bearer {TOKEN}"))));
    assert!(check_auth(Some(TOKEN), Some(&format!("bearer {TOKEN}"))));
}

#[test]
fn routes_map_paths_and_methods() {
    assert_eq!(route("GET", "/health"), Routed::Health);
    assert_eq!(route("GET", "/tasks"), Routed::Tasks);
    assert_eq!(route("GET", "/events"), Routed::Events);
    assert_eq!(route("POST", "/tasks"), Routed::MethodNotAllowed);
    assert_eq!(route("DELETE", "/health"), Routed::MethodNotAllowed);
    assert_eq!(route("GET", "/"), Routed::NotFound);
    assert_eq!(route("GET", "/tasks/extra"), Routed::NotFound);
}

#[test]
fn parse_strips_query_and_finds_authorization_case_insensitively() {
    let head = b"GET /tasks?format=json HTTP/1.1\r\nHost: localhost\r\nauthorization:  Bearer abc \r\n\r\n";
    let parsed = parse_request(head).unwrap();
    assert_eq!(parsed.method, "GET");
    assert_eq!(parsed.path, "/tasks");
    assert_eq!(parsed.authorization.as_deref(), Some("Bearer abc"));
}

#[test]
fn parse_rejects_malformed_heads() {
    assert!(parse_request(b"GET /tasks\r\n\r\n").is_err());
    assert!(parse_request(b"GET /tasks SPDY/3\r\n\r\n").is_err());
    assert!(parse_request(b" /tasks HTTP/1.1\r\n\r\n").is_err());
    assert!(parse_request(&[0xff, 0xfe, b' ']).is_err());
    let oversized = vec![b'a'; MAX_HEAD_BYTES + 1];
    assert!(parse_request(&oversized).is_err());
}

#[test]
fn reply_rejects_before_routing_when_token_missing() {
    let called = Cell::new(false);
    let outcome = reply(
        &request("GET", "/tasks", None),
        Some(TOKEN),
        "1.0.0",
        || {
            called.set(true);
            Ok(Vec::new())
        },
    );
    assert_eq!(
        outcome,
        Reply::Json {
            status: 401,
            body: error_body("unauthorized"),
        }
    );
    assert!(
        !called.get(),
        "tasks must not be read without a valid token"
    );

    // An unknown path also gets 401 without a token, so routes are not revealed.
    let unknown = reply(&request("GET", "/nope", None), Some(TOKEN), "1.0.0", || {
        Ok(Vec::new())
    });
    assert!(matches!(unknown, Reply::Json { status: 401, .. }));
}

#[test]
fn reply_serves_health_with_version() {
    let outcome = reply(
        &request("GET", "/health", Some(&format!("Bearer {TOKEN}"))),
        Some(TOKEN),
        "9.8.7",
        || Ok(Vec::new()),
    );
    assert_eq!(
        outcome,
        Reply::Json {
            status: 200,
            body: r#"{"status":"ok","version":"9.8.7"}"#.to_owned(),
        }
    );
}

#[test]
fn reply_maps_unknown_and_wrong_method_routes() {
    let missing = reply(&request("GET", "/x", None), None, "1", || Ok(Vec::new()));
    assert!(matches!(missing, Reply::Json { status: 404, .. }));
    let wrong = reply(&request("POST", "/tasks", None), None, "1", || {
        Ok(Vec::new())
    });
    assert!(matches!(wrong, Reply::Json { status: 405, .. }));
}

#[test]
fn reply_returns_events_marker_and_500_on_task_error() {
    assert_eq!(
        reply(&request("GET", "/events", None), None, "1", || Ok(
            Vec::new()
        )),
        Reply::Events
    );
    let failed = reply(&request("GET", "/tasks", None), None, "1", || {
        Err("boom".to_owned())
    });
    assert_eq!(
        failed,
        Reply::Json {
            status: 500,
            body: error_body("boom"),
        }
    );
}

#[test]
fn tasks_body_counts_every_status_and_lists_items() {
    let refs = vec![
        task(Status::Todo, "a"),
        task(Status::Done, "b"),
        task(Status::Todo, "c"),
    ];
    let value: Value = serde_json::from_str(&tasks_body(&refs)).unwrap();
    assert_eq!(value["count"], 3);
    assert_eq!(value["by_status"]["todo"], 2);
    assert_eq!(value["by_status"]["done"], 1);
    assert_eq!(value["by_status"]["blocked"], 0);
    assert_eq!(value["tasks"][0]["name"], "a");
    assert_eq!(value["tasks"][0]["status"], "todo");
    assert_eq!(value["tasks"][1]["title"], "Title of b");
}

#[test]
fn tasks_body_is_empty_for_an_empty_board() {
    let value: Value = serde_json::from_str(&tasks_body(&[])).unwrap();
    assert_eq!(value["count"], 0);
    assert_eq!(value["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn sse_event_frames_multiline_data_and_ends_with_blank_line() {
    let frame = sse_event("tasks", "line one\nline two\r\n");
    assert_eq!(
        frame,
        "event: tasks\ndata: line one\ndata: line two\ndata: \n\n"
    );
}

#[test]
fn sse_event_name_cannot_break_the_frame() {
    let frame = sse_event("tasks\r\ndata: injected", "{}");
    assert!(frame.starts_with("event: tasksdata: injected\n"));
    assert_eq!(frame.matches('\n').count(), 3);
}

#[test]
fn sse_comment_stays_on_one_line() {
    assert_eq!(sse_comment("keepalive"), ": keepalive\n\n");
    assert_eq!(sse_comment("a\nb\rc"), ": a b c\n\n");
}

#[test]
fn board_summary_reads_task_folders() {
    let temp = tempfile::tempdir_in(std::fs::canonicalize(std::env::temp_dir()).unwrap()).unwrap();
    let tasks_dir = temp.path().join(".plaesy").join("tasks");
    let task_dir = tasks_dir.join("todo").join("sample_task");
    fs::create_dir_all(&task_dir).unwrap();
    fs::write(
        task_dir.join(tasks::TASK_FILE),
        "---\ntitle: Sample task\nspec: sample-spec\n---\n",
    )
    .unwrap();

    let refs = tasks::list_all_tasks(&tasks_dir).unwrap();
    let value: Value = serde_json::from_str(&tasks_body(&refs)).unwrap();
    assert_eq!(value["count"], 1);
    assert_eq!(value["by_status"]["todo"], 1);
    assert_eq!(value["tasks"][0]["name"], "sample_task");
}
