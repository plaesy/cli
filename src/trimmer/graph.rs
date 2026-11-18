//! Chooses the Layer 2 level from a file's degree in the project graph.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::rules::Rules;

#[derive(Deserialize)]
struct GraphDoc {
    #[serde(default)]
    nodes: Vec<GraphNode>,
}

#[derive(Deserialize)]
struct GraphNode {
    id: String,
    #[serde(default)]
    degree: u64,
}

/// The degree of a file in `project.graph.json`, matched with either path separator.
#[must_use]
pub fn node_degree(graph_path: &Path, relative: &str) -> Option<u64> {
    let text = fs::read_to_string(graph_path).ok()?;
    let doc: GraphDoc = serde_json::from_str(&text).ok()?;
    let forward = relative.replace('\\', "/");
    let backward = relative.replace('/', "\\");
    doc.nodes
        .iter()
        .find(|n| n.id == relative || n.id == forward || n.id == backward)
        .map(|n| n.degree)
}

/// The level to use. An explicit request always wins. Otherwise the graph degree picks
/// `lite`, `full`, or `ultra` by the rule thresholds, and `full` is used when no degree is known.
#[must_use]
pub fn resolve_level(rules: &Rules, graph_path: &Path, relative: &str, requested: &str) -> String {
    if !requested.is_empty() {
        return requested.to_owned();
    }
    let Some(degree) = node_degree(graph_path, relative) else {
        return "full".to_owned();
    };
    let thresholds = rules.layer2_degree_thresholds;
    if degree >= u64::from(thresholds.lite_min_degree) {
        "lite".to_owned()
    } else if degree >= u64::from(thresholds.full_min_degree) {
        "full".to_owned()
    } else {
        "ultra".to_owned()
    }
}

#[cfg(test)]
#[path = "graph_test.rs"]
mod graph_test;
