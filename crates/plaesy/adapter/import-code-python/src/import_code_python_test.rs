use super::*;

const SAMPLE: &str = r#"
from dataclasses import dataclass
from enum import Enum
from typing import List, Optional

@dataclass
class OrderLine:
    product_id: str
    quantity: int

@dataclass
class Order:
    id: str
    customer_id: str
    total: float
    lines: List[OrderLine]
    note: Optional[str] = None
    paid: bool = False

class Status(Enum):
    DRAFT = "draft"
    PAID = "paid"
    SHIPPED = "shipped"

def helper():
    return 1
"#;

#[test]
fn python_maps_classes_fields_and_optional_defaults() {
    let imported = import(SAMPLE, "orders").unwrap();
    let types = imported.ir["types"].as_object().unwrap();
    let order = &types["Order"]["object"];
    assert_eq!(order["properties"]["total"], json!({ "type": "number" }));
    assert_eq!(
        order["properties"]["lines"],
        json!({ "array": { "$ref": "OrderLine" } })
    );
    assert_eq!(order["properties"]["note"], json!({ "type": "string" }));
    // Field opsional atau dengan default tidak wajib.
    assert_eq!(
        order["required"],
        json!(["id", "customer_id", "total", "lines"])
    );
}

#[test]
fn python_maps_enum_members_from_assignments() {
    let imported = import(SAMPLE, "orders").unwrap();
    assert_eq!(
        imported.ir["types"]["Status"],
        json!({ "enum": ["DRAFT", "PAID", "SHIPPED"] })
    );
}

#[test]
fn python_reports_functions_and_foreign_bases_as_losses_with_lines() {
    let imported = import(SAMPLE, "orders").unwrap();
    let helper = imported
        .report
        .lost
        .iter()
        .find(|l| l.reason.contains("fungsi"))
        .expect("fungsi harus tercatat");
    assert!(helper.source.contains("def helper"));
    assert!(helper.line > 0);
    assert!(imported.report.mapped.iter().any(|m| m.name == "Order"));
}

#[test]
fn python_unknown_annotation_is_a_loss_not_a_guess() {
    let imported = import(
        "from datetime import datetime\nclass A:\n    when: datetime\n    n: int\n",
        "a",
    )
    .unwrap();
    let properties = &imported.ir["types"]["A"]["object"]["properties"];
    assert!(
        properties.get("when").is_none(),
        "anotasi asing tidak boleh ditebak"
    );
    assert_eq!(properties["n"], json!({ "type": "integer" }));
    assert!(imported
        .report
        .lost
        .iter()
        .any(|l| l.reason.contains("when")));
}

#[test]
fn python_methods_are_losses() {
    let imported = import(
        "class A:\n    n: int\n    def f(self):\n        return 1\n",
        "a",
    )
    .unwrap();
    assert!(imported
        .report
        .lost
        .iter()
        .any(|l| l.reason.contains("metode f")));
}

#[test]
fn python_output_passes_the_api_ir_semantic_rules() {
    let imported = import(SAMPLE, "orders").unwrap();
    let errors = plaesy_domain_api::validate(&imported.ir);
    assert!(
        errors.is_empty(),
        "hasil impor harus lolos api-ir: {errors:?}"
    );
}

#[test]
fn python_rejects_source_that_does_not_parse() {
    assert!(import("class {", "x").is_err());
}
