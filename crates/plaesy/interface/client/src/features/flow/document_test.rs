use super::*;
use crate::features::flow::model::{modules_from_flow_json, ModuleView};
use serde_json::json;

const FIXTURE: &str = include_str!("../../platform/web/kirim-faktur.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("fixture JSON valid")
}

fn modules() -> Vec<ModuleView> {
    modules_from_flow_json(FIXTURE).expect("fixture dapat dibaca").1
}

fn patched_module<'a>(doc: &'a Value, id: &str) -> &'a Value {
    doc["modules"]
        .as_array()
        .expect("modules berupa array")
        .iter()
        .find(|m| m["id"] == id)
        .expect("modul ada")
}

#[test]
fn unchanged_modules_reproduce_the_original_document() {
    let original = fixture();
    assert_eq!(patch_document(&original, &modules()), original);
}

#[test]
fn removed_module_is_dropped_and_others_keep_their_fields() {
    let original = fixture();
    let mut edited = modules();
    edited.retain(|m| m.id != "m7");
    let doc = patch_document(&original, &edited);
    assert_eq!(doc["modules"].as_array().map(Vec::len), Some(edited.len()));
    assert_eq!(patched_module(&doc, "m1")["retry"]["max_attempts"], 3);
    assert_eq!(patched_module(&doc, "m1")["timeout_secs"], 30);
}

#[test]
fn edited_input_replaces_the_text_value() {
    let original = fixture();
    let mut edited = modules();
    let m1 = edited.iter_mut().find(|m| m.id == "m1").expect("m1 ada");
    m1.inputs = vec![("id".to_owned(), "flow.input.approver".to_owned())];
    let doc = patch_document(&original, &edited);
    assert_eq!(patched_module(&doc, "m1")["inputs"]["id"], "flow.input.approver");
}

#[test]
fn non_text_inputs_from_the_original_are_kept() {
    let original = json!({
        "name": "x",
        "modules": [{ "id": "m1", "kind": "action", "inputs": { "id": "flow.input.a", "meta": { "k": 1 } } }]
    });
    let edited = vec![ModuleView {
        id: "m1".to_owned(),
        kind: "action".to_owned(),
        operation: String::new(),
        inputs: vec![("id".to_owned(), "flow.input.b".to_owned())],
    }];
    let doc = patch_document(&original, &edited);
    assert_eq!(doc["modules"][0]["inputs"]["meta"], json!({ "k": 1 }));
    assert_eq!(doc["modules"][0]["inputs"]["id"], "flow.input.b");
}

#[test]
fn new_module_has_only_the_editable_fields() {
    let original = fixture();
    let mut edited = modules();
    edited.push(ModuleView {
        id: "m11".to_owned(),
        kind: "pass".to_owned(),
        operation: String::new(),
        inputs: Vec::new(),
    });
    let doc = patch_document(&original, &edited);
    let added = patched_module(&doc, "m11");
    assert_eq!(added, &json!({ "id": "m11", "kind": "pass" }));
}

#[test]
fn module_without_inputs_stays_without_inputs() {
    let original = fixture();
    let doc = patch_document(&original, &modules());
    assert!(patched_module(&doc, "m2").get("inputs").is_none());
}
