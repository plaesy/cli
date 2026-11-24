use super::*;
use serde_json::json;
use std::path::PathBuf;

fn example(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/adr/examples/app-ir-1")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn bindings_literal_param_and_query_column_resolve() {
    let p = params(&[("customer_id", "c-1")]);
    let queries: BTreeMap<String, Vec<Value>> = BTreeMap::from([(
        "qry_customer_order_totals".to_owned(),
        vec![json!({ "customer_name": "Budi", "total": 10 })],
    )]);
    let ctx = Context {
        params: &p,
        queries: &queries,
    };
    assert_eq!(
        resolve_value(&json!({ "literal": "Judul" }), &ctx).unwrap(),
        json!("Judul")
    );
    assert_eq!(
        resolve_value(&json!({ "param": "customer_id" }), &ctx).unwrap(),
        json!("c-1")
    );
    assert_eq!(
        resolve_value(
            &json!({ "query": "qry_customer_order_totals", "column": "customer_name" }),
            &ctx
        )
        .unwrap(),
        json!("Budi")
    );
}

#[test]
fn bindings_missing_param_and_column_are_errors() {
    let p = params(&[]);
    let queries = BTreeMap::new();
    let ctx = Context {
        params: &p,
        queries: &queries,
    };
    assert_eq!(
        resolve_value(&json!({ "param": "order_id" }), &ctx),
        Err(BindingError::MissingParam("order_id".to_owned()))
    );
    assert!(matches!(
        resolve_value(&json!({ "query": "q", "column": "c" }), &ctx),
        Err(BindingError::MissingQueryValue { .. })
    ));
}

#[test]
fn bindings_flow_output_is_pending_until_the_flow_runs() {
    let p = params(&[]);
    let queries = BTreeMap::new();
    let ctx = Context {
        params: &p,
        queries: &queries,
    };
    assert!(matches!(
        resolve_value(&json!({ "flow_output": "name", "flow_id": "flw_a" }), &ctx),
        Err(BindingError::FlowOutputPending { .. })
    ));
}

#[test]
fn bindings_unknown_source_is_rejected() {
    let p = params(&[]);
    let queries = BTreeMap::new();
    let ctx = Context {
        params: &p,
        queries: &queries,
    };
    assert!(matches!(
        resolve_value(&json!({ "tidak": "dikenal" }), &ctx),
        Err(BindingError::UnknownSource(_))
    ));
}

#[test]
fn bindings_fire_table_click_resolves_flow_inputs_from_params() {
    let doc = example("order-desk.json");
    let p = params(&[
        ("customer_id", "c-9"),
        ("order_id", "o-1"),
        ("status", "draft"),
    ]);
    let queries = BTreeMap::new();
    let ctx = Context {
        params: &p,
        queries: &queries,
    };
    let call = fire(&doc, "scr_orders", "cmp_table", "click", &ctx).unwrap();
    assert_eq!(call.flow_id, "flw_01J9OPENROW");
    assert_eq!(call.flow_version, Some(1));
    assert_eq!(call.inputs.get("customer"), Some(&json!("c-9")));
    let nav = call.on_success.expect("navigasi sukses harus ada");
    assert_eq!(nav.screen, "scr_customer");
}

#[test]
fn bindings_navigation_params_resolve_after_flow_outputs_are_known() {
    let doc = example("order-desk.json");
    let p = params(&[
        ("customer_id", "c-9"),
        ("order_id", "o-1"),
        ("status", "draft"),
    ]);
    let queries = BTreeMap::new();
    let ctx = Context {
        params: &p,
        queries: &queries,
    };
    let call = fire(&doc, "scr_orders", "cmp_table", "click", &ctx).unwrap();
    let outputs = BTreeMap::from([("customer_id".to_owned(), json!("c-9-keluaran"))]);
    let nav = resolve_navigation(&call.on_success.unwrap(), &ctx, &outputs).unwrap();
    assert_eq!(nav.screen, "scr_customer");
    assert_eq!(nav.params.get("customer_id"), Some(&json!("c-9-keluaran")));
}

#[test]
fn bindings_fire_on_unknown_event_is_no_such_event() {
    let doc = example("order-desk.json");
    let p = params(&[]);
    let queries = BTreeMap::new();
    let ctx = Context {
        params: &p,
        queries: &queries,
    };
    assert!(matches!(
        fire(&doc, "scr_orders", "cmp_table", "hover", &ctx),
        Err(BindingError::NoSuchEvent { .. })
    ));
}

#[test]
fn bindings_data_refs_list_query_bound_props() {
    let doc = example("order-desk.json");
    let refs = data_refs(&doc);
    assert!(refs.iter().any(|r| r.component == "cmp_table"
        && r.prop == "rows"
        && r.query == "qry_customer_order_totals"
        && r.column.as_deref() == Some("customer_name")));
}

#[test]
fn bindings_example_document_passes_app_rules() {
    let doc = example("order-desk.json");
    let errors = crate::validate(&doc);
    assert!(errors.is_empty(), "contoh app-ir harus lolos: {errors:?}");
}
