use super::*;

const CRM_SCHEMA: &str = "
CREATE TABLE customers (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    tier TEXT NOT NULL DEFAULT 'basic',
    created_at DATETIME NOT NULL
);
CREATE TABLE orders (
    id TEXT NOT NULL PRIMARY KEY,
    customer_id TEXT NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
    total NUMERIC(12, 2) NOT NULL,
    paid BOOLEAN NOT NULL DEFAULT 0,
    note TEXT
);
CREATE INDEX orders_customer_idx ON orders (customer_id);
";

fn import_schema(sql: &str) -> Result<Value, ImportError> {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(sql).unwrap();
    import(&conn, "crm")
}

fn table<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc["tables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == name)
        .unwrap()
}

fn column<'a>(table: &'a Value, name: &str) -> &'a Value {
    table["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap()
}

#[test]
fn import_crm_schema_is_valid_data_ir() {
    let doc = import_schema(CRM_SCHEMA).unwrap();
    assert_eq!(doc["ir"], "data-ir/1");
    let errors = plaesy_domain_data::validate(&doc);
    assert!(
        errors.is_empty(),
        "hasil impor harus lolos aturan data-ir: {errors:?}"
    );
}

#[test]
fn import_maps_sqlite_types_to_data_ir_types() {
    let doc = import_schema(CRM_SCHEMA).unwrap();
    let orders = table(&doc, "orders");
    assert_eq!(column(orders, "customer_id")["type"], "text");
    assert_eq!(column(orders, "paid")["type"], "boolean");
    assert_eq!(
        column(orders, "total")["type"],
        json!({ "decimal": { "precision": 12, "scale": 2 } })
    );
    assert_eq!(
        column(table(&doc, "customers"), "created_at")["type"],
        "timestamp"
    );
}

#[test]
fn import_reports_real_as_unsupported_instead_of_guessing() {
    let error =
        import_schema("CREATE TABLE sensor (id TEXT PRIMARY KEY, reading REAL);").unwrap_err();
    assert_eq!(
        error,
        ImportError::UnsupportedType {
            table: "sensor".to_owned(),
            column: "reading".to_owned(),
            declared: "REAL".to_owned(),
        }
    );
}

#[test]
fn import_foreign_key_becomes_one_to_many_with_on_delete() {
    let doc = import_schema(CRM_SCHEMA).unwrap();
    let relation = &doc["relations"][0];
    assert_eq!(relation["kind"], "one_to_many");
    assert_eq!(
        relation["parent"],
        json!({ "table": "customers", "columns": ["id"] })
    );
    assert_eq!(
        relation["child"],
        json!({ "table": "orders", "columns": ["customer_id"] })
    );
    assert_eq!(relation["on_delete"], "cascade");
}

#[test]
fn import_default_no_action_is_restrict() {
    let doc = import_schema(
        "CREATE TABLE a (id TEXT PRIMARY KEY);
         CREATE TABLE b (id TEXT PRIMARY KEY, a_id TEXT REFERENCES a(id));",
    )
    .unwrap();
    assert_eq!(doc["relations"][0]["on_delete"], "restrict");
}

#[test]
fn import_keeps_defaults_nullability_and_unique_constraints() {
    let doc = import_schema(CRM_SCHEMA).unwrap();
    let customers = table(&doc, "customers");
    assert_eq!(column(customers, "tier")["default"], "basic");
    assert_eq!(column(customers, "email")["unique"], true);
    assert_eq!(column(customers, "name")["nullable"], false);
    assert_eq!(column(table(&doc, "orders"), "note")["nullable"], true);
    assert_eq!(column(table(&doc, "orders"), "paid")["default"], 0);
}

#[test]
fn import_records_secondary_index_but_not_the_primary_key_index() {
    let doc = import_schema(CRM_SCHEMA).unwrap();
    let indexes = table(&doc, "orders")["indexes"].as_array().unwrap();
    assert_eq!(indexes.len(), 1);
    assert_eq!(indexes[0]["name"], "orders_customer_idx");
    assert_eq!(indexes[0]["columns"], json!(["customer_id"]));
    assert_eq!(indexes[0]["unique"], false);
}

#[test]
fn import_composite_primary_key_keeps_declared_order() {
    let doc = import_schema(
        "CREATE TABLE line (order_id TEXT, product_id TEXT, qty INTEGER, PRIMARY KEY (product_id, order_id));",
    )
    .unwrap();
    assert_eq!(
        table(&doc, "line")["primary_key"],
        json!(["product_id", "order_id"])
    );
}
