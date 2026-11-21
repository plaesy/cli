//! Unit tests for `tests` split out of `queue.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_finding_is_listed_again_only_when_its_state_changes() {
    let mut q = Queue::default();
    let f = |state: &str| Finding {
        id: "F1".into(),
        state: state.into(),
    };
    assert!(q.record(f("open")));
    assert!(!q.record(f("open")));
    assert!(q.record(f("fixed")));
    assert_eq!(q.len(), 1, "a changed finding replaces its queued entry");
    assert_eq!(q.take_next().unwrap().state, "fixed");
}
