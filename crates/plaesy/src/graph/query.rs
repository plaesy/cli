//! Read-only queries over a loaded graph, and the small JSON files some of them write.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use chrono::Local;
use serde::Serialize;

use crate::common::log::Logger;

use super::types::{Edge, Graph, Node};
use super::GraphError;

/// Lookup structures built once from a graph, so each query does not rescan it.
struct Index<'a> {
    by_id: BTreeMap<&'a str, &'a Node>,
    /// Node ids in file order.
    order: Vec<&'a str>,
    adj_out: BTreeMap<&'a str, Vec<&'a Edge>>,
    adj_in: BTreeMap<&'a str, Vec<&'a Edge>>,
    neighbors: BTreeMap<&'a str, BTreeSet<&'a str>>,
}

impl<'a> Index<'a> {
    fn new(g: &'a Graph) -> Self {
        let mut ix = Index {
            by_id: BTreeMap::new(),
            order: Vec::with_capacity(g.nodes.len()),
            adj_out: BTreeMap::new(),
            adj_in: BTreeMap::new(),
            neighbors: BTreeMap::new(),
        };
        for n in &g.nodes {
            ix.by_id.insert(n.id.as_str(), n);
            ix.order.push(n.id.as_str());
        }
        for e in &g.edges {
            ix.adj_out.entry(e.source.as_str()).or_default().push(e);
            ix.adj_in.entry(e.target.as_str()).or_default().push(e);
            ix.neighbors
                .entry(e.source.as_str())
                .or_default()
                .insert(e.target.as_str());
            ix.neighbors
                .entry(e.target.as_str())
                .or_default()
                .insert(e.source.as_str());
        }
        ix
    }

    /// The first node whose id contains `needle`, case-sensitive, in file order.
    fn find_node(&self, needle: &str) -> Option<&'a str> {
        self.order.iter().copied().find(|id| id.contains(needle))
    }

    /// Sorted, unique neighbours in either direction.
    fn neighbors_of(&self, id: &str) -> Vec<&'a str> {
        self.neighbors
            .get(id)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }
}

/// Case-insensitive substring match on node ids, with each match's neighbours.
///
/// # Errors
///
/// Writing to `out` fails.
pub fn do_query(g: &Graph, query: &str, out: &mut dyn Write, log: &Logger) -> io::Result<()> {
    let ix = Index::new(g);
    let ql = query.to_lowercase();
    let matches: Vec<&str> = ix
        .order
        .iter()
        .copied()
        .filter(|id| id.to_lowercase().contains(&ql))
        .collect();
    if matches.is_empty() {
        log.warning(&format!("No nodes matched '{query}'."));
        return Ok(());
    }
    for m in matches {
        let n = ix.by_id[m];
        writeln!(out, "\n== {m} ({}, degree {}) ==", n.kind, n.degree)?;
        let nbs = ix.neighbors_of(m);
        if nbs.is_empty() {
            writeln!(out, "  (no connections found)")?;
            continue;
        }
        for nb in nbs {
            writeln!(out, "  -> {nb}")?;
        }
    }
    Ok(())
}

/// Incoming and outgoing edges of the first node matching `name`.
///
/// # Errors
///
/// Writing to `out` fails.
pub fn do_explain(g: &Graph, name: &str, out: &mut dyn Write, log: &Logger) -> io::Result<()> {
    let ix = Index::new(g);
    let Some(id) = ix.find_node(name) else {
        log.warning(&format!("No node matching '{name}'."));
        return Ok(());
    };
    let n = ix.by_id[id];
    writeln!(out, "\n{id}")?;
    writeln!(
        out,
        "  type: {}   group: {}   degree: {}",
        n.kind, n.group, n.degree
    )?;
    if !n.symbols.is_empty() {
        writeln!(
            out,
            "  symbols ({}): {}",
            n.symbols.len(),
            n.symbols.join(", ")
        )?;
    }
    writeln!(out, "  outgoing:")?;
    let outgoing = ix.adj_out.get(id).map(Vec::as_slice).unwrap_or_default();
    if outgoing.is_empty() {
        writeln!(out, "    (none)")?;
    }
    for e in outgoing {
        writeln!(out, "    -[{}]-> {}", e.kind, e.target)?;
    }
    writeln!(out, "  incoming:")?;
    let incoming = ix.adj_in.get(id).map(Vec::as_slice).unwrap_or_default();
    if incoming.is_empty() {
        writeln!(out, "    (none)")?;
    }
    for e in incoming {
        writeln!(out, "    <-[{}]- {}", e.kind, e.source)?;
    }
    Ok(())
}

/// Shortest path, by breadth-first search, between two nodes found by substring.
///
/// # Errors
///
/// Writing to `out` fails.
pub fn do_path_query(
    g: &Graph,
    from: &str,
    to: &str,
    out: &mut dyn Write,
    log: &Logger,
) -> io::Result<()> {
    let ix = Index::new(g);
    let (Some(f), Some(t)) = (ix.find_node(from), ix.find_node(to)) else {
        log.warning("Could not resolve one or both node names.");
        return Ok(());
    };
    let mut prev: BTreeMap<&str, &str> = BTreeMap::new();
    let mut visited: BTreeSet<&str> = BTreeSet::from([f]);
    let mut queue = VecDeque::from([f]);
    while let Some(cur) = queue.pop_front() {
        if cur == t {
            break;
        }
        for nb in ix.neighbors_of(cur) {
            if visited.insert(nb) {
                prev.insert(nb, cur);
                queue.push_back(nb);
            }
        }
    }
    if !visited.contains(t) {
        log.warning(&format!("No path found between {f} and {t}."));
        return Ok(());
    }
    let mut path = vec![t];
    let mut cur = t;
    while cur != f {
        cur = prev[cur];
        path.push(cur);
    }
    path.reverse();
    writeln!(
        out,
        "[plaesy-graph] Path ({} nodes, {} hops):",
        path.len(),
        path.len() - 1
    )?;
    writeln!(out, "{}", path.join("  ->  "))?;
    Ok(())
}

/// Files reachable from the first node matching `name`, grouped by distance up to `depth`.
///
/// # Errors
///
/// Writing to `out` fails.
pub fn do_impact_check(
    g: &Graph,
    name: &str,
    depth: usize,
    out: &mut dyn Write,
    log: &Logger,
) -> io::Result<()> {
    let ix = Index::new(g);
    let Some(id) = ix.find_node(name) else {
        log.warning(&format!("No node matching '{name}'."));
        return Ok(());
    };
    let mut depth_of: BTreeMap<&str, usize> = BTreeMap::from([(id, 0)]);
    let mut queue = VecDeque::from([id]);
    while let Some(cur) = queue.pop_front() {
        let d = depth_of[cur];
        if d >= depth {
            continue;
        }
        for nb in ix.neighbors_of(cur) {
            if !depth_of.contains_key(nb) {
                depth_of.insert(nb, d + 1);
                queue.push_back(nb);
            }
        }
    }
    depth_of.remove(id);

    writeln!(
        out,
        "\n[plaesy-graph] Impact of changing {id} (depth {depth}):"
    )?;
    if depth_of.is_empty() {
        writeln!(
            out,
            "  No connected files found - safe to change in isolation (per detected edges)."
        )?;
        return Ok(());
    }
    for d in 1..=depth {
        let at: Vec<&str> = depth_of
            .iter()
            .filter(|(_, &dd)| dd == d)
            .map(|(k, _)| *k)
            .collect();
        if at.is_empty() {
            continue;
        }
        writeln!(out, "  Depth {d}:")?;
        for k in at {
            writeln!(out, "    - {k}  [{}]", ix.by_id[k].kind)?;
        }
    }
    Ok(())
}

/// Up to ten nodes whose id contains `query`, case-insensitively, with type, layer and degree.
///
/// # Errors
///
/// Writing to `out` fails.
pub fn do_fuzzy_query(g: &Graph, query: &str, out: &mut dyn Write) -> io::Result<()> {
    let ql = query.to_lowercase();
    for n in g
        .nodes
        .iter()
        .filter(|n| n.id.to_lowercase().contains(&ql))
        .take(10)
    {
        let layer = if n.layer.is_empty() {
            "n/a"
        } else {
            n.layer.as_str()
        };
        writeln!(
            out,
            "  - {} (type: {}, layer: {layer}, connections: {})",
            n.id, n.kind, n.degree
        )?;
    }
    Ok(())
}

#[derive(Serialize)]
struct SemanticItem<'a> {
    source: &'a str,
    target: &'a str,
    #[serde(rename = "type")]
    kind: &'a str,
}

/// Writes every INFERRED edge to `semantic-queue.json`. Returns the count; writes nothing when zero.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_semantic_queue(g: &Graph, out_full: &Path) -> Result<usize, GraphError> {
    let items: Vec<SemanticItem> = g
        .edges
        .iter()
        .filter(|e| e.confidence == "INFERRED")
        .map(|e| SemanticItem {
            source: &e.source,
            target: &e.target,
            kind: &e.kind,
        })
        .collect();
    if items.is_empty() {
        return Ok(0);
    }
    write_json_line(&out_full.join("semantic-queue.json"), &items)?;
    Ok(items.len())
}

#[derive(Serialize)]
struct BusinessItem {
    source: String,
    target: String,
    question: String,
}

/// Writes every INFERRED edge as a question to `business-logic-queue.json`. Returns the count.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_business_logic_queue(g: &Graph, out_full: &Path) -> Result<usize, GraphError> {
    let items: Vec<BusinessItem> = g
        .edges
        .iter()
        .filter(|e| e.confidence == "INFERRED")
        .map(|e| BusinessItem {
            source: e.source.clone(),
            target: e.target.clone(),
            question: format!("Business logic connecting {} to {}?", e.source, e.target),
        })
        .collect();
    write_json_line(&out_full.join("business-logic-queue.json"), &items)?;
    Ok(items.len())
}

#[derive(Serialize)]
struct LearningPaths<'a> {
    generated_at: String,
    total_nodes: usize,
    strategy: &'a str,
    entry_points: Vec<&'a str>,
}

/// Writes a stub `learning-paths.json`: the first five nodes as entry points.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_learning_paths(g: &Graph, out_full: &Path) -> Result<(), GraphError> {
    let doc = LearningPaths {
        generated_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        total_nodes: g.nodes.len(),
        strategy: "Analyze dependencies to suggest learning order",
        entry_points: g.nodes.iter().take(5).map(|n| n.id.as_str()).collect(),
    };
    write_json_line(&out_full.join("learning-paths.json"), &doc)
}

const IMPACT_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Impact Analysis</title>
  <style>
    body { font-family: Arial; margin: 20px; background: #f5f5f5 }
    .impact-tree { background: white; padding: 20px; border-radius: 8px; max-width: 800px }
    .node { padding: 10px; margin: 6px 0; background: #f9f9f9; border-left: 3px solid #2196F3; font-family: monospace }
    .depth-1 { margin-left: 20px }
    .depth-2 { margin-left: 40px }
    .layer { display: inline-block; padding: 2px 8px; border-radius: 3px; color: white; font-size: 11px; margin-left: 8px }
    .api { background: #4CAF50 }
    .service { background: #2196F3 }
    .data { background: #FF9800 }
    .ui { background: #E91E63 }
    .utility { background: #9C27B0 }
  </style>
</head>
<body>
  <h1>Impact Analysis Visualization</h1>
  <div class="impact-tree">
    <p>Depth analysis configured for: {{NODE}}</p>
    <p>Run: <code>plaesy analyze --impact-check NODE --impact-depth N</code> for detailed report</p>
  </div>
</body>
</html>
"#;

/// Writes a placeholder `impact-visualization.html` pointing at `--impact-check`.
///
/// The node name is HTML-escaped: a file named like markup must not become live markup.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_impact_visualization(node: &str, out_full: &Path) -> Result<(), GraphError> {
    fs::write(
        out_full.join("impact-visualization.html"),
        IMPACT_TEMPLATE.replace("{{NODE}}", &escape_html(node)),
    )?;
    Ok(())
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&#34;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

fn write_json_line<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), GraphError> {
    let mut json = serde_json::to_string_pretty(value)?;
    json.push('\n');
    fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
