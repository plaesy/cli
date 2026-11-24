use super::*;
use serde_json::json;

#[test]
fn json_identical_documents_have_no_changes() {
    let doc = json!({"a": [1, 2], "b": {"c": true}});
    assert!(diff(&doc, &doc).is_empty());
}

#[test]
fn json_reports_added_removed_and_changed_with_pointer_paths() {
    let before = json!({"name": "lama", "tags": ["x"], "gone": 1});
    let after = json!({"name": "baru", "tags": ["x", "y"], "new": true});
    let changes = diff(&before, &after);
    assert!(changes.contains(&Change::Changed {
        path: "/name".to_owned(),
        from: json!("lama"),
        to: json!("baru"),
    }));
    assert!(changes.contains(&Change::Added {
        path: "/tags/1".to_owned(),
        value: json!("y"),
    }));
    assert!(changes.contains(&Change::Removed {
        path: "/gone".to_owned(),
        value: json!(1),
    }));
    assert!(changes.contains(&Change::Added {
        path: "/new".to_owned(),
        value: json!(true),
    }));
}

#[test]
fn json_pointer_escapes_slash_and_tilde() {
    let before = json!({"a/b": 1, "c~d": 1});
    let after = json!({"a/b": 2, "c~d": 2});
    let changes = diff(&before, &after);
    let paths: Vec<&str> = changes.iter().map(Change::path).collect();
    assert_eq!(paths, vec!["/a~1b", "/c~0d"]);
}

#[test]
fn json_output_is_sorted_by_path() {
    let changes = diff(&json!({"b": 1, "a": 1}), &json!({"b": 2, "a": 2}));
    let paths: Vec<&str> = changes.iter().map(Change::path).collect();
    assert_eq!(paths, vec!["/a", "/b"]);
}
