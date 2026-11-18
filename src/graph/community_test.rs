//! Unit tests for `tests` split out of `community.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

fn edge(a: &str, b: &str) -> Edge {
    Edge {
        source: a.into(),
        target: b.into(),
        kind: "imports".into(),
        confidence: "EXTRACTED".into(),
    }
}

#[test]
fn a_triangle_is_one_community_and_an_isolated_node_is_its_own() {
    let ids: Vec<String> = ["a", "b", "c", "z"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let edges = vec![edge("a", "b"), edge("b", "c"), edge("a", "c")];
    let label = label_propagation(&ids, &build_adjacency(&edges));
    let community = assign_community_ids(&ids, &label);
    assert_eq!(community["a"], community["b"]);
    assert_eq!(community["a"], community["c"]);
    assert_ne!(community["a"], community["z"]);
    assert_eq!(compute_degree(&ids, &edges)["a"], 2);
}
