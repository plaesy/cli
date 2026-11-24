//! Layout sederhana berlapis: node ditempatkan per lapisan topologi (lapisan = jarak terpanjang dari sumber).
//! Graf dengan siklus tidak bisa diurutkan dan menghasilkan [`CycleError`].

use super::graph::Graph;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleError {
    pub nodes: Vec<String>,
}

/// Lapisan node. Lapisan 0 berisi node tanpa pendahulu. Setiap lapisan terurut menurut id.
///
/// # Errors
///
/// [`CycleError`] bila graf mengandung siklus.
pub fn layers(graph: &Graph) -> Result<Vec<Vec<String>>, CycleError> {
    let ids: Vec<String> = graph.nodes().map(|n| n.id.clone()).collect();
    let mut indegree: BTreeMap<String, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
    let mut successors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for edge in graph.edges() {
        *indegree.entry(edge.to.clone()).or_insert(0) += 1;
        successors
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let mut level: BTreeMap<String, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
    let mut queue: Vec<String> = indegree
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut processed = 0usize;
    while let Some(id) = queue.pop() {
        processed += 1;
        let current_level = level.get(&id).copied().unwrap_or(0);
        for next in successors.get(&id).cloned().unwrap_or_default() {
            let entry = level.entry(next.clone()).or_insert(0);
            *entry = (*entry).max(current_level + 1);
            if let Some(degree) = indegree.get_mut(&next) {
                *degree -= 1;
                if *degree == 0 {
                    queue.push(next);
                }
            }
        }
    }

    if processed != ids.len() {
        let nodes = indegree
            .iter()
            .filter(|(_, d)| **d > 0)
            .map(|(id, _)| id.clone())
            .collect();
        return Err(CycleError { nodes });
    }

    let depth = level.values().copied().max().map_or(0, |m| m + 1);
    let mut out = vec![Vec::new(); depth];
    for (id, lvl) in &level {
        out[*lvl].push(id.clone());
    }
    Ok(out)
}

/// Menempatkan node: x = lapisan * `spacing_x`, y = urutan dalam lapisan * `spacing_y`.
///
/// # Errors
///
/// [`CycleError`] bila graf mengandung siklus. Graf tidak diubah.
pub fn apply(graph: &mut Graph, spacing_x: f64, spacing_y: f64) -> Result<(), CycleError> {
    let layered = layers(graph)?;
    for (lvl, row) in layered.iter().enumerate() {
        for (index, id) in row.iter().enumerate() {
            graph.move_node(id, lvl as f64 * spacing_x, index as f64 * spacing_y);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "layout_test.rs"]
mod layout_test;
