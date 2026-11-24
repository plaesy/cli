use super::*;
use crate::shared::canvas::graph::Node;

fn with_node(id: &str) -> Graph {
    let mut g = Graph::default();
    g.add_node(Node {
        id: id.to_owned(),
        ..Node::default()
    })
    .unwrap();
    g
}

#[test]
fn history_undo_and_redo_restore_states() {
    let mut h = History::new(Graph::default());
    h.commit(with_node("a"));
    h.commit(with_node("b"));
    assert!(h.current().node("b").is_some());
    assert!(h.undo());
    assert!(h.current().node("a").is_some());
    assert!(h.redo());
    assert!(h.current().node("b").is_some());
}

#[test]
fn history_commit_clears_redo() {
    let mut h = History::new(Graph::default());
    h.commit(with_node("a"));
    h.undo();
    assert!(h.can_redo());
    h.commit(with_node("c"));
    assert!(!h.can_redo());
}

#[test]
fn history_undo_on_empty_is_false() {
    let mut h = History::new(Graph::default());
    assert!(!h.can_undo());
    assert!(!h.undo());
}
