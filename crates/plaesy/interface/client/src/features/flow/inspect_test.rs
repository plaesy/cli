use super::*;
use crate::features::flow::model::validate;
use crate::features::flow::reducer::{apply, Action};

fn module(id: &str, inputs: &[(&str, &str)]) -> ModuleView {
    ModuleView {
        id: id.to_owned(),
        kind: "action".to_owned(),
        operation: String::new(),
        inputs: inputs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

#[test]
fn candidates_exclude_the_target_and_keep_document_order() {
    let modules = vec![module("m1", &[]), module("m2", &[]), module("m3", &[])];
    assert_eq!(connect_candidates(&modules, "m2"), vec!["m1", "m3"]);
}

#[test]
fn source_expression_trims_and_refuses_empty_parts() {
    assert_eq!(
        source_expression(" m1 ", " invoice.id "),
        Some("m1.invoice.id".to_owned())
    );
    assert_eq!(source_expression("", "invoice"), None);
    assert_eq!(source_expression("m1", "   "), None);
}

#[test]
fn flagged_modules_mark_duplicates_and_unknown_sources_only() {
    let issues = vec![
        LocalIssue::DuplicateId("m1".into()),
        LocalIssue::UnknownSource {
            module: "m4".into(),
            source: "m9".into(),
        },
        LocalIssue::EmptyId,
    ];
    let flagged = flagged_modules(&issues);
    assert_eq!(
        flagged.into_iter().collect::<Vec<_>>(),
        vec!["m1".to_owned(), "m4".to_owned()]
    );
}

#[test]
fn connecting_to_a_missing_source_is_flagged_by_validation() {
    let base = vec![module("m1", &[]), module("m2", &[])];
    let connected = apply(
        &base,
        &Action::Connect {
            to: "m2".into(),
            input: "x".into(),
            source: source_expression("m9", "out").expect("ekspresi valid"),
        },
    )
    .expect("connect ke modul yang ada");
    let flagged = flagged_modules(&validate(&connected));
    assert!(flagged.contains("m2"));
}

#[test]
fn connecting_to_an_existing_source_clears_the_flag() {
    let base = vec![module("m1", &[]), module("m2", &[])];
    let connected = apply(
        &base,
        &Action::Connect {
            to: "m2".into(),
            input: "x".into(),
            source: source_expression("m1", "out").expect("ekspresi valid"),
        },
    )
    .expect("connect ke modul yang ada");
    assert!(flagged_modules(&validate(&connected)).is_empty());
}
