use std::path::{Path, PathBuf};

use serde_json::Value;

use super::lenient;
use super::validate;

fn adr(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("adr")
        .join(relative)
}

fn load(path: &Path) -> Value {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("baca {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("json {}: {e}", path.display()))
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("buka {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
}

const IRS: &[&str] = &[
    "flow-ir-1",
    "data-ir-1",
    "api-ir-1",
    "app-ir-1",
    "design-ir-1",
];

#[test]
fn every_example_passes_the_rust_validators() {
    for ir in IRS {
        let files = json_files(&adr(&format!("examples/{ir}")));
        assert!(!files.is_empty(), "contoh untuk {ir} kosong");
        for file in files {
            let doc = load(&file);
            assert_eq!(validate(&doc), Vec::<String>::new(), "{}", file.display());
        }
    }
}

#[test]
fn every_semantic_negative_is_rejected_by_the_rust_validators() {
    let mut checked = 0;
    for ir in IRS {
        for file in json_files(&adr(&format!("examples-negative-semantic/{ir}"))) {
            let doc = load(&file);
            assert!(
                !validate(&doc).is_empty(),
                "harus ditolak: {}",
                file.display()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 8, "delapan fixture negatif semantik diharapkan");
}

#[test]
fn corpus_documents_pass_the_rust_validators() {
    for file in json_files(&adr("corpus/design-ir-1")) {
        let doc = load(&file);
        assert_eq!(validate(&doc), Vec::<String>::new(), "{}", file.display());
    }
}

#[test]
fn unknown_ir_name_is_reported() {
    let doc: Value = serde_json::json!({ "ir": "tidak-ada/1" });
    assert_eq!(validate(&doc).len(), 1);
}

fn schema(ir: &str) -> Value {
    load(&adr(&format!("schemas/{ir}.schema.json")))
}

#[test]
fn lenient_moves_design_future_fields_into_extensions() {
    let mut doc = load(&adr("examples-lenient/design-ir-1/future-fields.json"));
    let warnings = lenient::apply(&schema("design-ir-1"), &mut doc);
    assert!(warnings.len() >= 2, "{warnings:?}");
    assert_eq!(
        doc["extensions"]["future_root_note"],
        "ditulis oleh versi lebih baru"
    );
    assert_eq!(validate(&doc), Vec::<String>::new());
}

#[test]
fn lenient_moves_unknown_flow_module_field_into_extensions() {
    let mut doc = load(&adr("examples/flow-ir-1/kirim-faktur.json"));
    doc["modules"][0]["future_hint"] = serde_json::json!("baru");
    let warnings = lenient::apply(&schema("flow-ir-1"), &mut doc);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(doc["modules"][0]["extensions"]["future_hint"], "baru");
    assert!(doc["modules"][0].get("future_hint").is_none());
    assert_eq!(validate(&doc), Vec::<String>::new());
}

#[test]
fn lenient_leaves_example_documents_unchanged() {
    for ir in IRS {
        for file in json_files(&adr(&format!("examples/{ir}"))) {
            let original = load(&file);
            let mut doc = original.clone();
            let warnings = lenient::apply(&schema(ir), &mut doc);
            assert!(warnings.is_empty(), "{}: {warnings:?}", file.display());
            assert_eq!(doc, original, "{}", file.display());
        }
    }
}
