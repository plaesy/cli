//! Unit tests for `tests` split out of `build.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

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
