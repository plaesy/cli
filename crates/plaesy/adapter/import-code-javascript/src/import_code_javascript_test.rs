use super::*;

const SAMPLE: &str = r#"
class OrderLine {
  productId: string;
  quantity: number;
}

class Order {
  id: string;
  customerId: string;
  total: number;
  lines: OrderLine[];
  note?: string;
  discount: number | null;
}

enum Status {
  Draft,
  Paid,
  Shipped,
}

function helper(): number {
  return 1;
}
"#;

#[test]
fn js_maps_classes_fields_arrays_and_optional_members() {
    let imported = import(SAMPLE, "orders").unwrap();
    let types = imported.ir["types"].as_object().unwrap();
    let order = &types["Order"]["object"];
    assert_eq!(order["properties"]["total"], json!({ "type": "number" }));
    assert_eq!(
        order["properties"]["lines"],
        json!({ "array": { "$ref": "OrderLine" } })
    );
    assert_eq!(order["properties"]["note"], json!({ "type": "string" }));
    assert_eq!(order["properties"]["discount"], json!({ "type": "number" }));
    // `note?` dan `discount: number | null` tidak wajib.
    assert_eq!(
        order["required"],
        json!(["id", "customerId", "total", "lines"])
    );
}

#[test]
fn js_maps_enum_members_without_initializers() {
    let imported = import(SAMPLE, "orders").unwrap();
    assert_eq!(
        imported.ir["types"]["Status"],
        json!({ "enum": ["Draft", "Paid", "Shipped"] })
    );
}

#[test]
fn js_reports_functions_with_line_and_source() {
    let imported = import(SAMPLE, "orders").unwrap();
    let helper = imported
        .report
        .lost
        .iter()
        .find(|l| l.reason.contains("fungsi"))
        .expect("fungsi harus tercatat");
    assert!(helper.source.contains("function helper"));
    assert!(helper.line > 0);
    assert!(imported.report.mapped.iter().any(|m| m.name == "Order"));
}

#[test]
fn js_unknown_type_is_a_loss_not_a_guess() {
    let imported = import("class A {\n  when: Date;\n  n: number;\n}\n", "a").unwrap();
    let properties = &imported.ir["types"]["A"]["object"]["properties"];
    assert!(
        properties.get("when").is_none(),
        "tipe asing tidak boleh ditebak"
    );
    assert_eq!(properties["n"], json!({ "type": "number" }));
    assert!(imported
        .report
        .lost
        .iter()
        .any(|l| l.reason.contains("when") || l.source.contains("when")));
}

#[test]
fn js_methods_are_losses() {
    let imported = import(
        "class A {\n  n: number;\n  f(): number { return 1; }\n}\n",
        "a",
    )
    .unwrap();
    assert!(imported
        .report
        .lost
        .iter()
        .any(|l| l.reason.contains("anggota kelas")));
}

#[test]
fn js_output_passes_the_api_ir_semantic_rules() {
    let imported = import(SAMPLE, "orders").unwrap();
    let errors = plaesy_domain_api::validate(&imported.ir);
    assert!(
        errors.is_empty(),
        "hasil impor harus lolos api-ir: {errors:?}"
    );
}

#[test]
fn js_rejects_source_that_does_not_parse() {
    assert!(import("class {", "x").is_err());
}
