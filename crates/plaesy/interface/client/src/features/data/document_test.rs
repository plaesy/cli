use super::*;
use serde_json::json;

fn crm_like() -> Value {
    json!({
        "ir": "data-ir/1",
        "id": "dat_01JA0CRM",
        "name": "crm",
        "tables": [
            {
                "name": "customers",
                "columns": [
                    { "name": "id", "type": "uuid", "nullable": false },
                    { "name": "tier", "type": { "enum": ["basic", "pro"] }, "nullable": false },
                    { "name": "total", "type": { "decimal": { "precision": 12, "scale": 2 } }, "nullable": true }
                ],
                "primary_key": ["id"]
            }
        ],
        "relations": [
            {
                "id": "rel_customer_orders",
                "kind": "one_to_many",
                "parent": { "table": "customers", "columns": ["id"] },
                "child": { "table": "orders", "columns": ["customer_id"] }
            },
            {
                "id": "rel_orders_products",
                "kind": "many_to_many",
                "left": { "table": "orders", "columns": ["id"] },
                "right": { "table": "products", "columns": ["id"] }
            }
        ]
    })
}

#[test]
fn reads_tables_columns_and_types() {
    let (tables, _) = tables_from_document(&crm_like()).expect("dokumen valid");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "customers");
    assert_eq!(tables[0].primary_key, vec!["id".to_owned()]);
    let types: Vec<&str> = tables[0]
        .columns
        .iter()
        .map(|c| c.data_type.as_str())
        .collect();
    assert_eq!(types, vec!["uuid", "enum(basic|pro)", "decimal(12,2)"]);
    assert!(tables[0].columns[2].nullable);
}

#[test]
fn many_to_many_uses_left_and_right_as_endpoints() {
    let (_, relations) = tables_from_document(&crm_like()).expect("dokumen valid");
    assert_eq!(relations.len(), 2);
    assert_eq!(
        relations[1].parent,
        ("orders".to_owned(), vec!["id".to_owned()])
    );
    assert_eq!(
        relations[1].child,
        ("products".to_owned(), vec!["id".to_owned()])
    );
}

#[test]
fn document_without_tables_is_refused() {
    assert!(tables_from_document(&json!({ "name": "x" })).is_err());
}

#[test]
fn unknown_column_type_is_labelled_not_guessed() {
    let doc = json!({
        "tables": [{ "name": "t", "columns": [{ "name": "c", "type": 7 }] }]
    });
    let (tables, _) = tables_from_document(&doc).expect("dokumen valid");
    assert_eq!(tables[0].columns[0].data_type, "tidak dikenal");
}

#[test]
fn document_name_uses_name_and_falls_back_to_id() {
    assert_eq!(
        document_name(&crm_like()),
        Some(("dat_01JA0CRM".to_owned(), "crm".to_owned()))
    );
    assert_eq!(
        document_name(&json!({ "id": "dat_x" })),
        Some(("dat_x".to_owned(), "dat_x".to_owned()))
    );
}

#[test]
fn load_and_save_use_the_data_document_path() {
    assert_eq!(
        load_request("dat_1").path,
        "/v1/documents/data-ir%2F1/dat_1"
    );
    let saved = save_request("dat_1", "{}");
    assert_eq!(saved.method, "PUT");
    assert_eq!(saved.body.as_deref(), Some("{}"));
}
