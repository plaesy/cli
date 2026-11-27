use super::*;
use serde_json::json;

fn document() -> Value {
    json!({
        "name": "x",
        "modules": [
            { "id": "m1", "kind": "action", "inputs": { "id": "flow.input.a" } },
            { "id": "m2", "kind": "action" }
        ]
    })
}

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
fn unchanged_modules_have_no_changes() {
    let original = document();
    let modules = vec![module("m1", &[("id", "flow.input.a")]), module("m2", &[])];
    assert!(document_changes(&original, &modules).is_empty());
}

#[test]
fn edited_input_is_one_changed_entry_at_its_path() {
    let original = document();
    let modules = vec![module("m1", &[("id", "flow.input.b")]), module("m2", &[])];
    let changes = document_changes(&original, &modules);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path(), "/modules/0/inputs/id");
    assert_eq!(
        describe_change(&changes[0]),
        "/modules/0/inputs/id: \"flow.input.a\" → \"flow.input.b\""
    );
}

#[test]
fn removed_module_shows_up_as_a_change() {
    let original = document();
    let modules = vec![module("m1", &[("id", "flow.input.a")])];
    let changes = document_changes(&original, &modules);
    assert!(changes.iter().any(|c| c.path().starts_with("/modules/1")));
}

#[test]
fn long_values_are_cut_to_the_limit() {
    let long = "a".repeat(200);
    let change = Change::Changed {
        path: "/p".to_owned(),
        from: json!(long),
        to: json!("b"),
    };
    let text = describe_change(&change);
    assert!(text.contains('…'));
    assert!(text.chars().count() < 100);
}
