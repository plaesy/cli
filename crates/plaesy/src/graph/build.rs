//! Builds the graph: structural, mention and mirror edges, degree and communities.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::common::log::Logger;

use super::community::{assign_community_ids, build_adjacency, compute_degree, label_propagation};
use super::incremental::file_results;
use super::mentions::drop_weak_mentions;
use super::scan::collect_files;
use super::types::{architecture_layer, node_type_of, Edge, Graph, Node};
use super::{out_dir_norm, GraphError, Options, Paths};

/// Scans the project and returns the full graph. Nothing is written here; see `render`.
///
/// # Errors
///
/// The output folder cannot be created, the tree cannot be walked, or no file was found.
pub fn build_graph(opts: &Options, p: &Paths, log: &Logger) -> Result<Graph, GraphError> {
    log.info(&format!("Scanning {} ...", p.repo_root.display()));
    fs::create_dir_all(&p.out_full)?;

    let files = collect_files(&p.repo_root, &out_dir_norm(p))?;
    if files.is_empty() {
        return Err(GraphError::NoFiles(p.repo_root.clone()));
    }
    let known: BTreeSet<String> = files.iter().cloned().collect();

    let results = file_results(opts, p, &files, &known, log);
    let mut edges = results.edges;
    edges.extend(mirror_edges(&files));

    let mut edges = drop_weak_mentions(edges);
    edges.sort();

    let degree = compute_degree(&files, &edges);
    let label = label_propagation(&files, &build_adjacency(&edges));
    let community = assign_community_ids(&files, &label);

    let nodes: Vec<Node> = files
        .iter()
        .map(|rel| Node {
            id: rel.clone(),
            label: base_name(rel).to_owned(),
            kind: node_type_of(rel).to_owned(),
            group: group_of(rel).to_owned(),
            degree: degree.get(rel).copied().unwrap_or(0),
            community: community.get(rel).copied().unwrap_or(0),
            layer: architecture_layer(rel).to_owned(),
            symbols: results.symbols.get(rel).cloned().unwrap_or_default(),
        })
        .collect();

    let root_name = p
        .repo_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Graph {
        generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        root: p.repo_root.to_string_lossy().into_owned(),
        description: format!(
            "Knowledge graph of {root_name} ({} files): source, docs, config and their references, calls, imports, and text mentions.",
            nodes.len()
        ),
        nodes,
        edges,
    })
}

/// Every pair of files that share a base name but sit in different folders.
fn mirror_edges(files: &[String]) -> Vec<Edge> {
    let mut by_base: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for rel in files {
        by_base.entry(base_name(rel)).or_default().push(rel);
    }
    let mut out = Vec::new();
    for members in by_base.values().filter(|m| m.len() >= 2) {
        // `files` is sorted, so each group is already in order.
        for (i, a) in members.iter().enumerate() {
            for b in &members[i + 1..] {
                out.push(Edge {
                    source: (*a).clone(),
                    target: (*b).clone(),
                    kind: "mirrors".into(),
                    confidence: "EXTRACTED".into(),
                });
            }
        }
    }
    out
}

#[must_use]
pub fn base_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn group_of(rel: &str) -> &str {
    rel.split('/').next().unwrap_or(rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::resolve_paths;

    #[test]
    fn a_markdown_link_becomes_a_references_edge() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("a.md"), "see [b](b.md)").expect("write a");
        fs::write(dir.path().join("b.md"), "# b").expect("write b");
        let opts = Options {
            repo_path: dir.path().to_path_buf(),
            out_dir: "out".into(),
            full: true,
        };
        let paths = resolve_paths(&opts).expect("paths");
        let log = Logger::new("test");
        let g = build_graph(&opts, &paths, &log).expect("build");
        assert_eq!(g.nodes.len(), 2);
        assert!(g
            .edges
            .iter()
            .any(|e| e.source == "a.md" && e.target == "b.md" && e.kind == "references"));
    }

    #[test]
    fn same_basename_in_two_folders_mirrors() {
        let files = vec!["a/x.md".to_owned(), "b/x.md".to_owned()];
        let edges = mirror_edges(&files);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].kind, "mirrors");
    }
}
