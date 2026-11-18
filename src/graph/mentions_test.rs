//! Unit tests for `tests` split out of `mentions.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_mention_is_dropped_when_a_stronger_edge_exists() {
    let strong = Edge {
        source: "a.md".into(),
        target: "b.md".into(),
        kind: "references".into(),
        confidence: "EXTRACTED".into(),
    };
    let weak = Edge {
        kind: "mentions".into(),
        confidence: "INFERRED".into(),
        ..strong.clone()
    };
    let out = drop_weak_mentions(vec![weak, strong.clone()]);
    assert_eq!(out, vec![strong]);
}

#[test]
fn windows_spelling_of_a_path_still_counts() {
    let cands = candidates(&["scripts/install.ps1".to_owned()]);
    let edges = mention_edges_for("README.md", "run scripts\\install.ps1", &cands);
    assert_eq!(edges.len(), 1);
}
