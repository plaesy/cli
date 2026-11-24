use super::*;

const SAMPLE: &str = "
pub struct Order {
    pub id: String,
    pub customer_id: String,
    pub total: f64,
    pub lines: Vec<OrderLine>,
    pub note: Option<String>,
}

pub struct OrderLine {
    pub product_id: String,
    pub quantity: i64,
}

pub enum Status {
    Draft,
    Paid,
    Shipped,
}

pub fn helper() -> u32 { 1 }

impl Order {
    pub fn total_owed(&self) -> f64 { self.total }
}
";

#[test]
fn import_maps_structs_enums_and_fields() {
    let imported = import(SAMPLE, "orders").unwrap();
    let types = imported.ir["types"].as_object().unwrap();
    assert_eq!(
        types["Status"],
        json!({ "enum": ["Draft", "Paid", "Shipped"] })
    );
    let order = &types["Order"]["object"];
    assert_eq!(order["properties"]["total"], json!({ "type": "number" }));
    assert_eq!(
        order["properties"]["lines"],
        json!({ "array": { "$ref": "OrderLine" } })
    );
    assert_eq!(order["properties"]["note"], json!({ "type": "string" }));
    // Option<T> tidak wajib; field lain wajib.
    assert_eq!(
        order["required"],
        json!(["id", "customer_id", "total", "lines"])
    );
}

#[test]
fn import_reports_what_it_could_not_map_with_line_and_source() {
    let imported = import(SAMPLE, "orders").unwrap();
    let lost: Vec<&str> = imported
        .report
        .lost
        .iter()
        .map(|l| l.reason.as_str())
        .collect();
    assert!(lost.iter().any(|r| r.contains("fungsi")), "{lost:?}");
    assert!(lost.iter().any(|r| r.contains("impl")), "{lost:?}");
    let helper = imported
        .report
        .lost
        .iter()
        .find(|l| l.reason.contains("fungsi"))
        .unwrap();
    assert!(helper.source.contains("helper"));
    assert!(helper.line > 0);
}

#[test]
fn import_mapped_list_names_each_mapped_construct() {
    let imported = import(SAMPLE, "orders").unwrap();
    let names: Vec<&str> = imported
        .report
        .mapped
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    assert_eq!(names, ["Order", "OrderLine", "Status"]);
}

#[test]
fn import_unknown_field_type_is_a_loss_not_a_guess() {
    let imported = import(
        "pub struct A { pub when: std::time::SystemTime, pub n: u32 }",
        "a",
    )
    .unwrap();
    let properties = &imported.ir["types"]["A"]["object"]["properties"];
    assert!(
        properties.get("when").is_none(),
        "tipe asing tidak boleh ditebak"
    );
    assert_eq!(properties["n"], json!({ "type": "integer" }));
    assert!(imported
        .report
        .lost
        .iter()
        .any(|l| l.reason.contains("when")));
}

#[test]
fn import_output_passes_the_api_ir_semantic_rules() {
    let imported = import(SAMPLE, "orders").unwrap();
    let errors = plaesy_domain_api::validate(&imported.ir);
    assert!(
        errors.is_empty(),
        "hasil impor harus lolos api-ir: {errors:?}"
    );
}

#[test]
fn import_export_import_gives_the_same_types() {
    let first = import(SAMPLE, "orders").unwrap();
    let emitted = export(&first.ir);
    let second = import(&emitted, "orders").unwrap();
    assert_eq!(first.ir["types"], second.ir["types"]);
}

#[test]
fn import_rejects_source_that_does_not_parse() {
    assert!(import("pub struct {", "x").is_err());
}
