use super::*;
use serde_json::json;

#[test]
fn trace_is_off_unless_the_variable_is_set() {
    assert_eq!(target_for(None), None);
    assert_eq!(target_for(Some("")), None);
    assert_eq!(target_for(Some("  ")), None);
    assert_eq!(target_for(Some("0")), None);
    assert_eq!(target_for(Some("off")), None);
}

#[test]
fn default_file_and_custom_paths() {
    assert_eq!(target_for(Some("1")), Some(PathBuf::from(DEFAULT_FILE)));
    assert_eq!(
        target_for(Some(" true ")),
        Some(PathBuf::from(DEFAULT_FILE))
    );
    assert_eq!(
        target_for(Some("C:/logs/chat.jsonl")),
        Some(PathBuf::from("C:/logs/chat.jsonl"))
    );
}

#[test]
fn append_writes_one_json_line_per_event_and_creates_the_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("trace.jsonl");
    append(
        &path,
        "chat.turn.sent",
        json!({ "provider": "claude-cli", "chars": 5 }),
    );
    append(&path, "chat.turn.ok", json!({ "wall_ms": 12 }));

    let text = fs::read_to_string(&path).expect("read trace");
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["event"], "chat.turn.sent");
    assert_eq!(lines[0]["provider"], "claude-cli");
    assert_eq!(lines[0]["chars"], 5);
    assert!(lines[0]["at_ms"].as_u64().unwrap_or(0) > 0);
    assert_eq!(lines[1]["event"], "chat.turn.ok");
}

#[test]
fn a_failed_write_is_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    // The path is a directory, so opening it for append fails; nothing panics.
    append(dir.path(), "chat.turn.error", json!({}));
}
