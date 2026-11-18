//! Plain-text mentions: a file that contains another node's path is linked to it.
//! A mention is weak. It is dropped when a stronger edge already joins the same pair.

use std::collections::{BTreeSet, HashSet};

use super::types::Edge;

/// A node id as a file may spell it: forward slashes, or backslashes for Windows paths.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub forward: String,
    pub backward: String,
}

#[must_use]
pub fn candidates(node_ids: &[String]) -> Vec<Candidate> {
    node_ids
        .iter()
        .map(|id| Candidate {
            forward: id.clone(),
            backward: id.replace('/', "\\"),
        })
        .collect()
}

/// The mention edges that start at `rel`, whose content is `content`.
#[must_use]
pub fn mention_edges_for(rel: &str, content: &str, cands: &[Candidate]) -> Vec<Edge> {
    if content.is_empty() {
        return Vec::new();
    }
    cands
        .iter()
        .filter(|c| c.forward != rel)
        .filter(|c| content.contains(&c.forward) || content.contains(&c.backward))
        .map(|c| Edge {
            source: rel.to_owned(),
            target: c.forward.clone(),
            kind: "mentions".into(),
            confidence: "INFERRED".into(),
        })
        .collect()
}

/// Drops a `mentions` edge whose pair already has a stronger edge, then removes duplicates.
#[must_use]
pub fn drop_weak_mentions(edges: Vec<Edge>) -> Vec<Edge> {
    let strong: HashSet<(String, String)> = edges
        .iter()
        .filter(|e| e.kind != "mentions")
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect();
    let mut seen: BTreeSet<Edge> = BTreeSet::new();
    let mut out = Vec::new();
    for e in edges {
        if e.kind == "mentions" && strong.contains(&(e.source.clone(), e.target.clone())) {
            continue;
        }
        if seen.insert(e.clone()) {
            out.push(e);
        }
    }
    out
}

#[cfg(test)]
#[path = "mentions_test.rs"]
mod mentions_test;
