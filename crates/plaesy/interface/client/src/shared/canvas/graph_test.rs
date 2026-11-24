use super::*;

fn node(id: &str) -> Node {
    Node {
        id: id.to_owned(),
        label: id.to_uppercase(),
        x: 0.0,
        y: 0.0,
    }
}

#[test]
fn graph_rejects_duplicate_and_unknown_nodes() {
    let mut g = Graph::default();
    g.add_node(node("a")).unwrap();
    assert_eq!(
        g.add_node(node("a")),
        Err(GraphError::DuplicateNode("a".to_owned()))
    );
    assert_eq!(
        g.connect("a", "b"),
        Err(GraphError::UnknownNode("b".to_owned()))
    );
}

#[test]
fn graph_connect_is_idempotent_and_rejects_self_loops() {
    let mut g = Graph::default();
    g.add_node(node("a")).unwrap();
    g.add_node(node("b")).unwrap();
    g.connect("a", "b").unwrap();
    g.connect("a", "b").unwrap();
    assert_eq!(g.edges().len(), 1);
    assert_eq!(
        g.connect("a", "a"),
        Err(GraphError::SelfLoop("a".to_owned()))
    );
}

#[test]
fn graph_remove_node_drops_its_edges() {
    let mut g = Graph::default();
    for id in ["a", "b", "c"] {
        g.add_node(node(id)).unwrap();
    }
    g.connect("a", "b").unwrap();
    g.connect("b", "c").unwrap();
    g.remove_node("b").unwrap();
    assert!(g.edges().is_empty());
    assert!(g.node("b").is_none());
}

#[test]
fn graph_move_reports_missing_node() {
    let mut g = Graph::default();
    g.add_node(node("a")).unwrap();
    assert!(g.move_node("a", 3.0, 4.0));
    assert_eq!(g.node("a").unwrap().x, 3.0);
    assert!(!g.move_node("zz", 0.0, 0.0));
}

#[test]
fn selection_toggle_and_clear() {
    let mut s = Selection::default();
    assert!(s.toggle("a"));
    assert!(s.contains("a"));
    assert!(!s.toggle("a"));
    s.select("b");
    s.clear();
    assert_eq!(s.ids().count(), 0);
}
