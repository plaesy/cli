//! Writers for the graph: `project.graph.json`, `reports.md` and `project.html`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use chrono::Local;
use serde::Deserialize;

use crate::common::log::Logger;

use super::types::{Graph, Node};
use super::GraphError;

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Plaesy Graph</title>
<style>
  body{margin:0;font-family:Segoe UI,Arial,sans-serif;background:#111;color:#eee;}
  #graph{width:100vw;height:100vh;display:block;}
  #panel{position:fixed;top:10px;left:10px;background:#1e1e1eee;padding:10px 14px;border-radius:8px;max-width:320px;font-size:13px;}
  #panel input{width:100%;box-sizing:border-box;margin-top:6px;}
</style></head>
<body>
<div id="panel"><b>Plaesy Knowledge Graph</b><br/><input id="search" placeholder="filter nodes..." /><div id="info" style="margin-top:8px;color:#aaa;"></div></div>
<canvas id="graph"></canvas>
<script>
const data = {{.JSON}};
const canvas = document.getElementById('graph');
const ctx = canvas.getContext('2d');
function resize(){canvas.width=window.innerWidth;canvas.height=window.innerHeight;}
window.addEventListener('resize', resize); resize();
const groups = [...new Set(data.nodes.map(n=>n.group))];
const colors = ['#4fc3f7','#81c784','#ffb74d','#e57373','#ba68c8','#4db6ac','#f06292','#a1887f','#90a4ae'];
const colorOf = g => colors[groups.indexOf(g) % colors.length];
const nodes = data.nodes.map(n => ({...n, x: Math.random()*canvas.width, y: Math.random()*canvas.height, vx:0, vy:0}));
const idx = {}; nodes.forEach((n,i)=>idx[n.id]=i);
const edges = data.edges.filter(e => idx[e.source]!==undefined && idx[e.target]!==undefined);
let filter = '';
document.getElementById('search').addEventListener('input', e => { filter = e.target.value.toLowerCase(); });
function tick(){
  const k = 0.002, rep = 800, damp = 0.85, center = 0.0005;
  for(let i=0;i<nodes.length;i++){
    for(let j=i+1;j<nodes.length;j++){
      const a=nodes[i], b=nodes[j];
      let dx=a.x-b.x, dy=a.y-b.y;
      let dist = Math.sqrt(dx*dx+dy*dy)||1;
      const f = rep/(dist*dist);
      dx/=dist; dy/=dist;
      a.vx += dx*f; a.vy += dy*f;
      b.vx -= dx*f; b.vy -= dy*f;
    }
  }
  edges.forEach(e=>{
    const a=nodes[idx[e.source]], b=nodes[idx[e.target]];
    let dx=b.x-a.x, dy=b.y-a.y;
    a.vx += dx*k; a.vy += dy*k;
    b.vx -= dx*k; b.vy -= dy*k;
  });
  nodes.forEach(n=>{
    n.vx += (canvas.width/2 - n.x)*center;
    n.vy += (canvas.height/2 - n.y)*center;
    n.vx*=damp; n.vy*=damp;
    n.x += n.vx; n.y += n.vy;
  });
}
function draw(){
  ctx.clearRect(0,0,canvas.width,canvas.height);
  ctx.strokeStyle = '#444';
  edges.forEach(e=>{
    const a=nodes[idx[e.source]], b=nodes[idx[e.target]];
    ctx.beginPath(); ctx.moveTo(a.x,a.y); ctx.lineTo(b.x,b.y); ctx.stroke();
  });
  nodes.forEach(n=>{
    const match = filter && n.id.toLowerCase().includes(filter);
    const r = 3 + Math.min(10, n.degree);
    ctx.beginPath();
    ctx.fillStyle = filter ? (match ? colorOf(n.group) : '#333') : colorOf(n.group);
    ctx.arc(n.x, n.y, r, 0, Math.PI*2); ctx.fill();
    if (match || n.degree > 4) {
      ctx.fillStyle = '#eee'; ctx.font = '10px sans-serif';
      ctx.fillText(n.label, n.x+r+2, n.y+3);
    }
  });
}
function loop(){ tick(); draw(); requestAnimationFrame(loop); }
loop();
canvas.addEventListener('mousemove', e=>{
  const mx=e.clientX, my=e.clientY;
  let closest=null, best=20;
  nodes.forEach(n=>{ const d=Math.hypot(n.x-mx,n.y-my); if(d<best){best=d;closest=n;} });
  document.getElementById('info').textContent = closest ? closest.id + ' (' + closest.type + ', degree ' + closest.degree + ')' : '';
});
</script>
</body></html>
"#;

/// Writes the graph as pretty-printed `project.graph.json`.
///
/// # Errors
///
/// The graph cannot be serialised or the file cannot be written.
pub fn save_json(g: &Graph, path: &Path) -> Result<(), GraphError> {
    let mut json = serde_json::to_string_pretty(g)?;
    json.push('\n');
    fs::write(path, json)?;
    Ok(())
}

/// Reads `project.graph.json` back, for the standalone query commands.
///
/// # Errors
///
/// The file cannot be read or is not a graph.
pub fn load_json(path: &Path) -> Result<Graph, GraphError> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

/// Writes `reports.md`, a plain-language summary. A non-empty `rationale` is appended as
/// an "Explained relationships" section.
///
/// # Errors
///
/// The file cannot be written.
pub fn write_report(g: &Graph, out_full: &Path, rationale: &str) -> Result<(), GraphError> {
    let mut out = String::new();
    let _ = write!(out, "# Plaesy Graph Report\n\n");
    let _ = write!(
        out,
        "Generated: {}\n\n",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    let _ = write!(
        out,
        "## Overview\n- Nodes: {}\n- Edges: {}\n\n",
        g.nodes.len(),
        g.edges.len()
    );

    let _ = writeln!(out, "## Nodes by type");
    for (key, count) in count_desc(g.nodes.iter().map(|n| n.kind.as_str())) {
        let _ = writeln!(out, "- {key}: {count}");
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Communities (top-level folders)");
    let groups = count_desc(g.nodes.iter().map(|n| n.group.as_str()));
    for (key, count) in &groups {
        let _ = writeln!(out, "- {key}: {count} files");
    }
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "## Detected communities (label-propagation clustering)"
    );
    let _ = writeln!(
        out,
        "These are computed from actual edge structure, not folder names - they surface cross-folder subsystems."
    );
    write_detected_communities(&mut out, g);
    let _ = writeln!(out);

    let _ = writeln!(out, "## God nodes (most connected)");
    let mut by_degree: Vec<&Node> = g.nodes.iter().collect();
    by_degree.sort_by_key(|n| std::cmp::Reverse(n.degree));
    for n in by_degree.iter().take(10) {
        let _ = writeln!(out, "- `{}` - degree {}", n.id, n.degree);
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Orphan files (no detected references)");
    let orphans: Vec<&str> = g
        .nodes
        .iter()
        .filter(|n| n.degree == 0)
        .map(|n| n.id.as_str())
        .collect();
    if orphans.is_empty() {
        let _ = writeln!(out, "- None");
    } else {
        for id in orphans.iter().take(30) {
            let _ = writeln!(out, "- `{id}`");
        }
        if orphans.len() > 30 {
            let _ = writeln!(out, "- ... and {} more", orphans.len() - 30);
        }
    }
    let _ = writeln!(out);

    if !rationale.is_empty() {
        let _ = write!(
            out,
            "## Explained relationships (semantic pass)\n{rationale}\n\n"
        );
    }

    let _ = writeln!(out, "## Suggested questions");
    let _ = writeln!(out, "- Which files are referenced by the most other files?");
    let _ = writeln!(out, "- Which files have no callers (potentially dead)?");
    if let [first, second, ..] = groups.as_slice() {
        let _ = writeln!(out, "- What connects `{}` to `{}`?", first.0, second.0);
    }

    fs::write(out_full.join("reports.md"), out)?;
    Ok(())
}

/// Writes the self-contained force-directed `project.html`, with the graph JSON embedded.
///
/// # Errors
///
/// The graph cannot be serialised or the file cannot be written.
pub fn write_html(g: &Graph, out_full: &Path) -> Result<(), GraphError> {
    let json = serde_json::to_string(g)?;
    fs::write(
        out_full.join("project.html"),
        HTML_TEMPLATE.replace("{{.JSON}}", &json),
    )?;
    Ok(())
}

/// One `{source, target, rationale}` object from an annotations file. Missing fields read as
/// empty so the validation below reports them by name.
#[derive(Debug, Deserialize)]
struct Annotation {
    #[serde(default)]
    source: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    rationale: String,
}

/// Folds a `[{source, target, rationale}]` file into the report, then rewrites the graph files.
///
/// Each object is parsed as a unit, so a missing rationale is an error rather than a
/// borrowed one. An annotation that matches no edge is skipped with a warning.
///
/// # Errors
///
/// The file is not valid JSON, an annotation is incomplete, or an output cannot be written.
pub fn apply_semantic(
    g: &Graph,
    annotations: &Path,
    out_full: &Path,
    log: &Logger,
) -> Result<usize, GraphError> {
    let text = fs::read_to_string(annotations)?;
    let anns: Vec<Annotation> = serde_json::from_str(&text).map_err(|e| {
        GraphError::Invalid(format!("parse annotations {}: {e}", annotations.display()))
    })?;

    let existing: BTreeSet<(&str, &str)> = g
        .edges
        .iter()
        .map(|e| (e.source.as_str(), e.target.as_str()))
        .collect();
    let mut rationale = String::new();
    let mut applied = 0;
    for (i, a) in anns.iter().enumerate() {
        if a.source.is_empty() || a.target.is_empty() {
            return Err(GraphError::Invalid(format!(
                "annotation {i}: source and target are both required (got source={:?} target={:?})",
                a.source, a.target
            )));
        }
        if a.rationale.is_empty() {
            return Err(GraphError::Invalid(format!(
                "annotation {i} ({} -> {}): rationale is required",
                a.source, a.target
            )));
        }
        if !existing.contains(&(a.source.as_str(), a.target.as_str())) {
            log.warning(&format!(
                "annotation {i}: no edge {} -> {} in the graph; skipped",
                a.source, a.target
            ));
            continue;
        }
        let _ = writeln!(
            rationale,
            "- `{}` -> `{}`: {}",
            a.source, a.target, a.rationale
        );
        applied += 1;
    }

    write_report(g, out_full, &rationale)?;
    save_json(g, &out_full.join("project.graph.json"))?;
    write_html(g, out_full)?;
    Ok(applied)
}

/// Counts each key and sorts by count descending, then key ascending.
fn count_desc<'a>(values: impl Iterator<Item = &'a str>) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for v in values {
        *counts.entry(v).or_insert(0) += 1;
    }
    let mut out: Vec<(String, usize)> =
        counts.into_iter().map(|(k, c)| (k.to_owned(), c)).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

/// Lists each community with more than one member, up to five members in degree order.
fn write_detected_communities(out: &mut String, g: &Graph) {
    let mut by_community: BTreeMap<usize, Vec<&Node>> = BTreeMap::new();
    for n in &g.nodes {
        by_community.entry(n.community).or_default().push(n);
    }
    for (cid, members) in by_community.iter_mut() {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|n| std::cmp::Reverse(n.degree));
        let names: Vec<&str> = members.iter().take(5).map(|n| n.id.as_str()).collect();
        let _ = writeln!(
            out,
            "- Community {cid} ({} files): {}",
            members.len(),
            names.join(", ")
        );
    }
}

#[cfg(test)]
mod tests {
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
}
