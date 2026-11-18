//! Unit tests for `tests` split out of `query.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::graph::types::Edge;

fn edge(a: &str, b: &str) -> Edge {
    Edge {
        source: a.into(),
        target: b.into(),
        kind: "imports".into(),
        confidence: "EXTRACTED".into(),
    }
}

fn node(id: &str, degree: usize) -> Node {
    Node {
        id: id.into(),
        label: id.into(),
        kind: "source".into(),
        group: id.split('/').next().unwrap_or(id).into(),
        degree,
        community: 0,
        layer: String::new(),
        symbols: Vec::new(),
    }
}

fn chain() -> Graph {
    Graph {
        generated_at: "t".into(),
        root: "/r".into(),
        description: "d".into(),
        nodes: vec![node("a.rs", 1), node("b.rs", 2), node("c.rs", 1)],
        edges: vec![edge("a.rs", "b.rs"), edge("b.rs", "c.rs")],
    }
}

#[test]
fn path_query_finds_the_shortest_chain() {
    let log = Logger::new("test");
    let mut out = Vec::new();
    do_path_query(&chain(), "a.rs", "c.rs", &mut out, &log).expect("write");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("a.rs  ->  b.rs  ->  c.rs"));
}

#[test]
fn impact_check_lists_nodes_by_depth() {
    let log = Logger::new("test");
    let mut out = Vec::new();
    do_impact_check(&chain(), "a.rs", 2, &mut out, &log).expect("write");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("Depth 1:"));
    assert!(text.contains("Depth 2:"));
}

#[test]
fn impact_visualization_escapes_the_node_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_impact_visualization("<img src=x onerror=1>", dir.path()).expect("write");
    let html = fs::read_to_string(dir.path().join("impact-visualization.html")).expect("read");
    assert!(html.contains("&lt;img src=x onerror=1&gt;"));
    assert!(!html.contains("<img src=x"));
}
