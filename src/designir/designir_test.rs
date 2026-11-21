use std::path::{Path, PathBuf};

use serde_json::Value;

use super::codegen::generate;
use super::lenient;
use super::roundtrip;
use super::validate::validate;
use super::DesignError;

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

fn json_files(relative: &str) -> Vec<PathBuf> {
    let dir = adr(relative);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("buka {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
}

#[test]
fn order_desk_example_passes_semantic_rules() {
    let doc = load(&adr("examples/design-ir-1/order-desk-ui.json"));
    assert_eq!(validate(&doc), Vec::<String>::new());
}

#[test]
fn every_semantic_negative_fixture_is_rejected() {
    let files = json_files("examples-negative-semantic/design-ir-1");
    assert_eq!(files.len(), 3, "tiga fixture negatif semantik diharapkan");
    for file in files {
        let doc = load(&file);
        assert!(
            !validate(&doc).is_empty(),
            "harus ditolak: {}",
            file.display()
        );
    }
}

#[test]
fn synthetic_corpus_has_twenty_valid_documents() {
    let files = json_files("corpus/design-ir-1");
    assert_eq!(files.len(), 20);
    for file in files {
        let doc = load(&file);
        assert_eq!(validate(&doc), Vec::<String>::new(), "{}", file.display());
    }
}

#[test]
fn codegen_refuses_an_invalid_document() {
    let doc = load(&adr(
        "examples-negative-semantic/design-ir-1/category-mismatch.json",
    ));
    assert!(matches!(generate(&doc), Err(DesignError::Invalid(_))));
}

#[test]
fn generated_module_has_design_id_and_one_line_per_node() {
    let doc = load(&adr("examples/design-ir-1/order-desk-ui.json"));
    let source = generate(&doc).expect("generate");
    assert!(source.contains("pub const DESIGN_ID: &str = \"dsg_order_desk\";"));
    let node_lines = source
        .lines()
        .filter(|l| l.trim_start().starts_with("NodeSpec {"))
        .count();
    assert_eq!(node_lines, roundtrip::check(&doc).expect("check").nodes);
}

#[test]
fn round_trip_is_structurally_exact_for_example() {
    let doc = load(&adr("examples/design-ir-1/order-desk-ui.json"));
    let report = roundtrip::check(&doc).expect("check");
    assert!(report.mismatches.is_empty(), "{:?}", report.mismatches);
    assert!(report.nodes > 0 && report.tokens > 0);
}

#[test]
fn round_trip_is_structurally_exact_for_every_corpus_document() {
    for file in json_files("corpus/design-ir-1") {
        let doc = load(&file);
        let report = roundtrip::check(&doc).expect("check");
        assert!(
            report.mismatches.is_empty(),
            "{}: {:?}",
            file.display(),
            report.mismatches
        );
        assert!(report.nodes > 0, "{}", file.display());
    }
}

#[test]
fn round_trip_detects_a_changed_parent() {
    let doc = load(&adr("examples/design-ir-1/order-desk-ui.json"));
    let source = generate(&doc).expect("generate");
    let tampered = source.replacen("parent: Some(\"nd_orders_frame\")", "parent: None", 1);
    let extracted = roundtrip::extract(&tampered).expect("extract");
    let expected = super::codegen::node_rows(&doc);
    assert_ne!(
        extracted.nodes, expected,
        "perubahan induk harus terdeteksi"
    );
}

#[test]
fn literal_parser_handles_escapes() {
    let (value, rest) = roundtrip::parse_literal("\"a\\\"b\\\\c\", x").expect("parse");
    assert_eq!(value, "a\"b\\c");
    assert_eq!(rest, ", x");
}

#[test]
fn lenient_moves_future_fields_into_extensions() {
    let mut doc = load(&adr("examples-lenient/design-ir-1/future-fields.json"));
    let warnings = lenient::apply(&mut doc);
    assert!(warnings.len() >= 2, "{warnings:?}");
    assert_eq!(
        doc["extensions"]["future_root_note"],
        "ditulis oleh versi lebih baru"
    );
    assert_eq!(validate(&doc), Vec::<String>::new());
}
