//! Unit tests for `tests` split out of `render.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::graph::types::Edge;

fn sample() -> Graph {
    Graph {
        generated_at: "t".into(),
        root: "/r".into(),
        description: "d".into(),
        nodes: vec![Node {
            id: "a.md".into(),
            label: "a.md".into(),
            kind: "doc".into(),
            group: "a.md".into(),
            degree: 1,
            community: 0,
            layer: String::new(),
            symbols: Vec::new(),
        }],
        edges: vec![Edge {
            source: "a.md".into(),
            target: "b.md".into(),
            kind: "references".into(),
            confidence: "EXTRACTED".into(),
        }],
    }
}

#[test]
fn a_missing_rationale_is_rejected_not_borrowed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ann = dir.path().join("ann.json");
    fs::write(&ann, r#"[{"source":"a.md","target":"b.md"}]"#).expect("write");
    let log = Logger::new("test");
    let err = apply_semantic(&sample(), &ann, dir.path(), &log).expect_err("must fail");
    assert!(err.to_string().contains("rationale is required"));
}

#[test]
fn count_desc_orders_by_count_then_key() {
    let out = count_desc(["b", "a", "b"].into_iter());
    assert_eq!(out, vec![("b".to_owned(), 2), ("a".to_owned(), 1)]);
}
