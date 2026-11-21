use std::path::{Path, PathBuf};

use serde_json::Value;

use super::dioxus_codegen::{generate, pascal_case};
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

fn sample_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("designir")
        .join("dioxus_sample.rs")
}

fn order_desk() -> Value {
    load(&adr("examples/design-ir-1/order-desk-ui.json"))
}

#[test]
fn pascal_case_maps_ids_to_component_names() {
    assert_eq!(pascal_case("cp_button"), "CpButton");
    assert_eq!(pascal_case("pg_orders"), "PgOrders");
    assert_eq!(pascal_case("cp_a-b.c"), "CpABC");
}

#[test]
fn generated_output_declares_one_component_per_component_and_page() {
    let source = generate(&order_desk()).expect("generate");
    assert!(source.contains("use dioxus::prelude::*;"));
    assert!(source.contains("#[component]\npub fn CpButton() -> Element {"));
    assert!(source.contains("#[component]\npub fn PgOrders() -> Element {"));
    assert!(source.contains("rsx! {"));
}

#[test]
fn every_node_id_appears_once_as_data_attribute() {
    let doc = order_desk();
    let source = generate(&doc).expect("generate");
    let rows = super::codegen::node_rows(&doc);
    for row in rows {
        let needle = format!("\"data-plaesy-id\": \"{}\"", row.id);
        assert_eq!(source.matches(&needle).count(), 1, "node {}", row.id);
    }
}

#[test]
fn text_is_written_as_expression_so_braces_are_safe() {
    let mut doc = order_desk();
    doc["pages"][0]["frames"][0]["children"][0]["text"] = Value::String("Halo {nama}".to_string());
    let source = generate(&doc).expect("generate");
    assert!(source.contains("{ \"Halo {nama}\" }"));
}

#[test]
fn colour_tokens_become_inline_style() {
    let source = generate(&order_desk()).expect("generate");
    assert!(source.contains("background-color: #2563EB"));
}

#[test]
fn generator_refuses_an_invalid_document() {
    let doc = load(&adr(
        "examples-negative-semantic/design-ir-1/category-mismatch.json",
    ));
    assert!(matches!(generate(&doc), Err(DesignError::Invalid(_))));
}

#[test]
fn committed_sample_matches_the_generator() {
    // Jika tes ini gagal: jalankan `cargo test --lib regenerate_dioxus_sample -- --ignored`.
    let committed = std::fs::read_to_string(sample_path()).expect("baca dioxus_sample.rs");
    let generated = generate(&order_desk()).expect("generate");
    assert_eq!(
        committed, generated,
        "dioxus_sample.rs tidak sama dengan keluaran generator"
    );
}

#[test]
#[ignore = "menulis ulang dioxus_sample.rs; jalankan secara manual"]
fn regenerate_dioxus_sample() {
    let generated = generate(&order_desk()).expect("generate");
    std::fs::write(sample_path(), generated).expect("tulis dioxus_sample.rs");
}

#[test]
fn every_synthetic_corpus_document_generates_all_its_nodes() {
    let dir = adr("corpus/design-ir-1");
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).expect("baca korpus") {
        let path = entry.expect("entri").path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let doc = load(&path);
        let source = generate(&doc).expect("generate");
        for row in super::codegen::node_rows(&doc) {
            let needle = format!("\"data-plaesy-id\": \"{}\"", row.id);
            assert!(source.contains(&needle), "{}: {}", path.display(), row.id);
        }
        count += 1;
    }
    assert_eq!(count, 20);
}
