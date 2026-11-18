//! Unit tests for `tests` split out of `tasks_validate.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn build_and_doc_items_are_exempt_from_the_behaviour_rule() {
    let mut item = crate::checklist::Item {
        id: "x".into(),
        check: String::new(),
        run: "cargo test".into(),
        expected: "pass".into(),
        status: crate::checklist::Status::Pending,
        histories: Vec::new(),
        receipt: None,
        evidence: None,
    };
    assert!(!is_exempt(&item));
    item.evidence = Some("build".into());
    assert!(is_exempt(&item));
    item.evidence = Some("doc".into());
    assert!(is_exempt(&item));
    item.evidence = Some("other".into());
    assert!(!is_exempt(&item));
}

#[test]
fn a_plaesy_segment_counts_as_behaviour_evidence() {
    assert!(invokes_plaesy("cargo build && plaesy status"));
    assert!(invokes_plaesy("\"plaesy.exe\" validate"));
    assert!(!invokes_plaesy("go test ./..."));
}
