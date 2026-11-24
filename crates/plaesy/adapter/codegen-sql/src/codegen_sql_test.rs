use super::*;
use rusqlite::types::{ToSql, Value as SqlValue};
use rusqlite::Connection;
use serde_json::json;
use std::path::PathBuf;

fn example(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/adr/examples/data-ir-1")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn query_named(doc: &Value, name: &str) -> CompiledQuery {
    queries(doc)
        .unwrap()
        .into_iter()
        .find(|q| q.name == name)
        .unwrap_or_else(|| panic!("query {name} tidak ada"))
}

/// Mengikat parameter deklarasi (teks atau angka) dan literal dari query, lalu menjalankannya.
fn run(conn: &Connection, query: &CompiledQuery, threshold: i64) -> usize {
    let mut owned: Vec<(String, SqlValue)> = Vec::new();
    // Hanya parameter yang muncul di SQL yang diikat; SQLite menolak nama yang tidak ada.
    for param in query
        .params
        .iter()
        .filter(|p| query.sql.contains(&format!(":{p}")))
    {
        let value = if param == "threshold" {
            SqlValue::Integer(threshold)
        } else {
            SqlValue::Text("paid".to_owned())
        };
        owned.push((format!(":{param}"), value));
    }
    for (name, literal) in query
        .literals
        .iter()
        .filter(|(n, _)| query.sql.contains(&format!(":{n}")))
    {
        let value = match literal {
            Value::String(s) => SqlValue::Text(s.clone()),
            Value::Number(n) => SqlValue::Integer(n.as_i64().unwrap_or(0)),
            Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
            _ => SqlValue::Null,
        };
        owned.push((format!(":{name}"), value));
    }
    let bindings: Vec<(&str, &dyn ToSql)> = owned
        .iter()
        .map(|(n, v)| (n.as_str(), v as &dyn ToSql))
        .collect();
    let mut statement = conn
        .prepare(&query.sql)
        .unwrap_or_else(|e| panic!("{}: {e}\n{}", query.name, query.sql));
    let mut rows = statement.query(bindings.as_slice()).unwrap();
    let mut count = 0;
    while rows.next().unwrap().is_some() {
        count += 1;
    }
    count
}

#[test]
fn sql_crm_ddl_executes_in_sqlite() {
    let conn = Connection::open_in_memory().unwrap();
    let script = ddl(&example("crm.json")).unwrap();
    conn.execute_batch(&script)
        .unwrap_or_else(|e| panic!("DDL gagal: {e}\n{script}"));
}

#[test]
fn sql_inventory_ddl_with_many_to_many_executes_in_sqlite() {
    let conn = Connection::open_in_memory().unwrap();
    let script = ddl(&example("inventory.json")).unwrap();
    conn.execute_batch(&script)
        .unwrap_or_else(|e| panic!("DDL gagal: {e}\n{script}"));
}

#[test]
fn sql_ddl_maps_types_checks_defaults_and_foreign_keys() {
    let script = ddl(&example("crm.json")).unwrap();
    assert!(script.contains("CREATE TABLE \"customers\""));
    assert!(script.contains(
        "\"tier\" TEXT NOT NULL DEFAULT 'basic' CHECK (\"tier\" IN ('basic', 'pro', 'enterprise'))"
    ));
    assert!(script.contains("\"total\" NUMERIC"), "decimal -> NUMERIC");
    assert!(script.contains("\"created_at\" TEXT"), "timestamp -> TEXT");
    assert!(script.contains("\"email\" TEXT NOT NULL UNIQUE"));
    assert!(script.contains("PRIMARY KEY (\"id\")"));
    assert!(script.contains("CREATE INDEX \"customers_tier_idx\" ON \"customers\" (\"tier\");"));
}

#[test]
fn sql_relation_on_delete_mapping_and_one_to_one_unique() {
    let script = ddl(&example("inventory.json")).unwrap();
    assert!(
        script.contains("ON DELETE CASCADE"),
        "cascade dari rel_warehouse_stock"
    );
    assert!(
        script.contains("ON DELETE NO ACTION"),
        "restrict dipetakan ke NO ACTION"
    );
    // one_to_one: kolom anak wajib unik.
    assert!(script.contains("UNIQUE (\"warehouse_id\")"));
}

#[test]
fn sql_many_to_many_becomes_junction_table() {
    // crm.json punya rel_orders_products (many_to_many) lewat junction order_items.
    let script = ddl(&example("crm.json")).unwrap();
    assert!(script.contains("CREATE TABLE \"order_items\""));
    assert!(script.contains("PRIMARY KEY (\"order_id\", \"product_id\")"));
    assert!(script.contains("REFERENCES \"orders\" (\"id\")"));
    assert!(script.contains("REFERENCES \"products\" (\"id\")"));
}

#[test]
fn sql_order_totals_query_uses_bound_parameters_and_joins() {
    let doc = example("crm.json");
    let q = query_named(&doc, "customer_order_totals");
    assert!(q
        .sql
        .contains("LEFT JOIN \"orders\" ON \"customers\".\"id\" = \"orders\".\"customer_id\""));
    assert!(q
        .sql
        .contains("COUNT(\"orders\".\"id\") AS \"order_count\""));
    assert!(q.sql.contains("\"orders\".\"status\" = :status"));
    assert!(q.sql.contains("ORDER BY \"total_spent\" DESC"));
    assert!(q.sql.contains("LIMIT 50"));
    assert_eq!(q.params, vec!["status".to_owned()]);
    assert!(q.literals.is_empty());
}

#[test]
fn sql_order_totals_query_runs_with_named_parameter() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&ddl(&example("crm.json")).unwrap())
        .unwrap();
    let q = query_named(&example("crm.json"), "customer_order_totals");
    assert_eq!(
        run(&conn, &q, 0),
        0,
        "tabel kosong, query harus berjalan tanpa baris"
    );
}

#[test]
fn sql_inventory_query_runs_with_integer_parameter() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&ddl(&example("inventory.json")).unwrap())
        .unwrap();
    let q = query_named(&example("inventory.json"), "low_stock");
    assert!(q.params.contains(&"threshold".to_owned()));
    assert_eq!(run(&conn, &q, 10), 0);
}

#[test]
fn sql_literal_is_bound_never_inlined() {
    let mut doc = example("crm.json");
    let payload = "x'); DROP TABLE customers; --";
    doc["queries"][0]["where"] = json!({
        "cmp": {
            "left": { "table": "customers", "column": "name" },
            "op": "eq",
            "right": payload
        }
    });
    let q = query_named(&doc, "customer_order_totals");
    assert!(
        !q.sql.contains("DROP"),
        "nilai literal tidak boleh masuk ke SQL"
    );
    assert!(!q.sql.contains(payload));
    assert_eq!(
        q.literals,
        vec![("lit1".to_owned(), Value::String(payload.to_owned()))]
    );

    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(&ddl(&example("crm.json")).unwrap())
        .unwrap();
    assert_eq!(
        run(&conn, &q, 0),
        0,
        "literal diikat dengan aman dan query tetap berjalan"
    );
}

#[test]
fn sql_invalid_document_is_refused() {
    let mut doc = example("crm.json");
    doc["tables"] = json!([]);
    assert!(matches!(ddl(&doc), Err(SqlError::Invalid(_))));
}

#[test]
fn sql_quoting_escapes_double_quotes_in_identifiers() {
    assert_eq!(quote("a\"b"), "\"a\"\"b\"");
    assert_eq!(sql_string("it's"), "'it''s'");
}
