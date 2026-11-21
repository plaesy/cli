use super::*;
use serde_json::json;

const SAMPLE: &str = r#"package orders

import "fmt"

type OrderLine struct {
	ProductID string `json:"product_id"`
	Quantity  int    `json:"quantity"`
}

type Order struct {
	ID         string      `json:"id"`
	CustomerID string      `json:"customer_id"`
	Total      float64     `json:"total"`
	Lines      []OrderLine `json:"lines"`
	Note       *string     `json:"note"`
}

type Status string

const (
	StatusDraft   Status = "draft"
	StatusPaid    Status = "paid"
	StatusShipped Status = "shipped"
)

func helper() int {
	return 1
}
"#;

#[test]
fn go_maps_structs_fields_and_pointer_optional() {
    let imported = import(SAMPLE, "orders").unwrap();
    let types = imported.ir["types"].as_object().unwrap();
    let order = &types["Order"]["object"];
    // Properti memakai nama field Go, sama dengan daftar `required`.
    assert_eq!(order["properties"]["Total"], json!({ "type": "number" }));
    assert_eq!(
        order["properties"]["Lines"],
        json!({ "array": { "$ref": "OrderLine" } })
    );
    assert_eq!(order["properties"]["Note"], json!({ "type": "string" }));
    // Pointer (*string) opsional; field lain wajib.
    assert_eq!(
        order["required"],
        json!(["ID", "CustomerID", "Total", "Lines"])
    );
}

#[test]
fn go_maps_string_type_with_const_block_to_enum() {
    let imported = import(SAMPLE, "orders").unwrap();
    assert_eq!(
        imported.ir["types"]["Status"],
        json!({ "enum": ["draft", "paid", "shipped"] })
    );
}

#[test]
fn go_enum_with_non_string_member_is_lost_not_guessed() {
    let source = "package orders\n\ntype Level string\n\nconst (\n\tLow Level = \"low\"\n\tHigh Level = 2\n)\n";
    let imported = import(source, "orders").unwrap();
    assert!(imported.ir["types"].get("Level").is_none());
    assert!(imported
        .report
        .lost
        .iter()
        .any(|loss| loss.reason.contains("string tanpa const enum")));
}

#[test]
fn go_reports_functions_and_imports_as_losses_with_lines() {
    let imported = import(SAMPLE, "orders").unwrap();
    let reasons: Vec<&str> = imported
        .report
        .lost
        .iter()
        .map(|l| l.reason.as_str())
        .collect();
    assert!(reasons.iter().any(|r| r.contains("helper")), "{reasons:?}");
    assert!(reasons.iter().any(|r| r.contains("impor")), "{reasons:?}");
    let helper = imported
        .report
        .lost
        .iter()
        .find(|l| l.reason.contains("helper"))
        .unwrap();
    assert!(helper.line > 0);
}

#[test]
fn go_unknown_field_type_is_a_loss_not_a_guess() {
    let imported = import(
        "package a\n\ntype A struct {\n\tWhen time.Time\n\tN    int\n}\n",
        "a",
    )
    .unwrap();
    let properties = &imported.ir["types"]["A"]["object"]["properties"];
    assert!(
        properties.get("When").is_none(),
        "tipe asing tidak boleh ditebak"
    );
    assert_eq!(properties["N"], json!({ "type": "integer" }));
    assert!(imported
        .report
        .lost
        .iter()
        .any(|l| l.reason.contains("When")));
}

#[test]
fn go_embedded_struct_is_a_loss() {
    let imported = import(
        "package a\n\ntype Base struct{ ID string }\n\ntype A struct {\n\tBase\n\tN int\n}\n",
        "a",
    )
    .unwrap();
    assert!(imported
        .report
        .lost
        .iter()
        .any(|l| l.reason.contains("embedded")));
}

#[test]
fn go_output_passes_the_api_ir_semantic_rules() {
    let imported = import(SAMPLE, "orders").unwrap();
    let errors = plaesy_domain_api::validate(&imported.ir);
    assert!(
        errors.is_empty(),
        "hasil impor harus lolos api-ir: {errors:?}"
    );
}

#[test]
fn go_rejects_source_that_does_not_parse() {
    assert!(import("package {", "x").is_err());
}
