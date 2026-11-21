//! Unit tests for `tests` split out of `graph.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn an_explicit_level_wins_over_the_graph() {
    let rules = Rules::default();
    assert_eq!(
        resolve_level(&rules, Path::new("missing.json"), "a.md", "ultra"),
        "ultra"
    );
}

#[test]
fn a_missing_graph_means_full() {
    let rules = Rules::default();
    assert_eq!(
        resolve_level(&rules, Path::new("missing.json"), "a.md", ""),
        "full"
    );
}

#[test]
fn degree_picks_the_level_and_either_separator_matches() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("project.graph.json");
    fs::write(
        &path,
        r#"{"nodes":[{"id":"src\\hub.md","degree":9},{"id":"src/leaf.md","degree":0}]}"#,
    )
    .unwrap();
    let rules = Rules::default();
    assert_eq!(resolve_level(&rules, &path, "src/hub.md", ""), "lite");
    assert_eq!(resolve_level(&rules, &path, "src/leaf.md", ""), "ultra");
}
