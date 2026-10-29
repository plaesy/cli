//! Degree and label-propagation communities. Deterministic: ties break by label order.

use std::collections::{BTreeMap, BTreeSet};

use super::types::Edge;

/// Each edge adds one to the degree of both its endpoints, whatever its type.
#[must_use]
pub fn compute_degree(node_ids: &[String], edges: &[Edge]) -> BTreeMap<String, usize> {
    let mut degree: BTreeMap<String, usize> = node_ids.iter().map(|id| (id.clone(), 0)).collect();
    for e in edges {
        *degree.entry(e.source.clone()).or_insert(0) += 1;
        *degree.entry(e.target.clone()).or_insert(0) += 1;
    }
    degree
}

/// Undirected adjacency: each edge links both ways.
#[must_use]
pub fn build_adjacency(edges: &[Edge]) -> BTreeMap<String, Vec<String>> {
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in edges {
        adj.entry(e.source.clone())
            .or_default()
            .push(e.target.clone());
        adj.entry(e.target.clone())
            .or_default()
            .push(e.source.clone());
    }
    adj
}

/// Up to 15 rounds in which each node adopts the most common label among its neighbours.
/// Ties go to the smallest label.
#[must_use]
pub fn label_propagation(
    node_ids: &[String],
    adj: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, String> {
    let mut label: BTreeMap<String, String> =
        node_ids.iter().map(|id| (id.clone(), id.clone())).collect();
    for _ in 0..15 {
        let mut changed = false;
        for id in node_ids {
            let Some(neighbors) = adj.get(id).filter(|n| !n.is_empty()) else {
                continue;
            };
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for nb in neighbors {
                let l = label.get(nb).cloned().unwrap_or_else(|| nb.clone());
                *counts.entry(l).or_insert(0) += 1;
            }
            // BTreeMap iterates in label order, so the first label at the highest count wins.
            let mut best: Option<(&String, usize)> = None;
            for (l, &c) in &counts {
                if best.is_none_or(|(_, bc)| c > bc) {
                    best = Some((l, c));
                }
            }
            if let Some((best_label, _)) = best {
                if label.get(id).map(String::as_str) != Some(best_label.as_str()) {
                    label.insert(id.clone(), best_label.clone());
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    label
}

/// Renumbers labels as community ids: larger communities get lower ids, and ties go by label.
#[must_use]
pub fn assign_community_ids(
    node_ids: &[String],
    label: &BTreeMap<String, String>,
) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for id in node_ids {
        let l = label.get(id).cloned().unwrap_or_else(|| id.clone());
        *counts.entry(l).or_insert(0) += 1;
    }
    let mut labels: Vec<&String> = counts.keys().collect();
    labels.sort_by(|a, b| counts[*b].cmp(&counts[*a]).then(a.cmp(b)));
    let ids: BTreeMap<&String, usize> = labels.iter().enumerate().map(|(i, l)| (*l, i)).collect();
    node_ids
        .iter()
        .map(|id| {
            let l = label.get(id).cloned().unwrap_or_else(|| id.clone());
            let community = ids.get(&l).copied().unwrap_or(0);
            (id.clone(), community)
        })
        .collect()
}

/// Node ids that have at least one edge touching them. Used to test the edge set's shape.
#[must_use]
pub fn connected_nodes(edges: &[Edge]) -> BTreeSet<String> {
    edges
        .iter()
        .flat_map(|e| [e.source.clone(), e.target.clone()])
        .collect()
}

#[cfg(test)]
mod tests {
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
}
