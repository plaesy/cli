use super::*;
use crate::shared::canvas::graph::Node;

fn graph(ids: &[&str], edges: &[(&str, &str)]) -> Graph {
    let mut g = Graph::default();
    for id in ids {
        g.add_node(Node {
            id: (*id).to_owned(),
            ..Node::default()
        })
        .unwrap();
    }
    for (from, to) in edges {
        g.connect(from, to).unwrap();
    }
    g
}

#[test]
fn layout_places_longest_path_depth_as_layers() {
    let g = graph(&["a", "b", "c", "d"], &[("a", "b"), ("b", "c"), ("a", "d")]);
    let layered = layers(&g).unwrap();
    assert_eq!(layered[0], vec!["a".to_owned()]);
    assert!(layered[1].contains(&"b".to_owned()) && layered[1].contains(&"d".to_owned()));
    assert_eq!(layered[2], vec!["c".to_owned()]);
}

#[test]
fn layout_detects_cycles() {
    let g = graph(&["a", "b"], &[("a", "b"), ("b", "a")]);
    let err = layers(&g).unwrap_err();
    assert_eq!(err.nodes, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn layout_apply_moves_nodes_by_layer() {
    let mut g = graph(&["a", "b"], &[("a", "b")]);
    apply(&mut g, 100.0, 10.0).unwrap();
    assert_eq!(g.node("a").unwrap().x, 0.0);
    assert_eq!(g.node("b").unwrap().x, 100.0);
}
