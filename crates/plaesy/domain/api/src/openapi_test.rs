use super::*;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn example(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/adr/examples/api-ir-1")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// Mengumpulkan semua `$ref` di dokumen.
fn refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(target)) = map.get("$ref") {
                out.push(target.clone());
            }
            map.values().for_each(|v| refs(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| refs(v, out)),
        _ => {}
    }
}

/// flow_id yang dirujuk endpoint.
fn flow_ids(doc: &Value) -> HashSet<String> {
    doc["endpoints"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["handler"]["flow_id"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn openapi_orders_example_becomes_3_1_document() {
    let openapi = to_openapi(&example("orders.json")).unwrap();
    assert_eq!(openapi["openapi"], OPENAPI_VERSION);
    assert_eq!(openapi["info"]["title"], "orders");
    let paths = openapi["paths"].as_object().unwrap();
    assert!(
        paths.contains_key("/v1/orders/{order_id}"),
        "path dari base_path + path"
    );
    assert_eq!(
        paths["/v1/orders/{order_id}"]["get"]["operationId"],
        "ep_get_order"
    );
    assert!(
        paths.contains_key("/v1/orders"),
        "path '/' memakai base_path saja"
    );
    assert!(openapi["components"]["schemas"].get("Order").is_some());
}

#[test]
fn openapi_path_params_become_path_parameters() {
    let openapi = to_openapi(&example("orders.json")).unwrap();
    let params = &openapi["paths"]["/v1/orders/{order_id}"]["get"]["parameters"];
    assert_eq!(params[0]["name"], "order_id");
    assert_eq!(params[0]["in"], "path");
    assert_eq!(params[0]["required"], true);
}

#[test]
fn openapi_every_ref_resolves_to_a_component_schema() {
    for name in ["orders.json", "auth-and-errors.json"] {
        let openapi = to_openapi(&example(name)).unwrap();
        let schemas: BTreeSet<String> = openapi["components"]["schemas"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let mut found = Vec::new();
        refs(&openapi, &mut found);
        for target in found {
            let name = target
                .strip_prefix("#/components/schemas/")
                .unwrap_or_else(|| panic!("ref tidak lokal: {target}"));
            assert!(schemas.contains(name), "{target} tidak punya skema");
        }
    }
}

#[test]
fn openapi_operations_have_unique_ids_and_responses() {
    let openapi = to_openapi(&example("auth-and-errors.json")).unwrap();
    let mut ids = BTreeSet::new();
    for item in openapi["paths"].as_object().unwrap().values() {
        for op in item.as_object().unwrap().values() {
            let id = op["operationId"].as_str().unwrap();
            assert!(ids.insert(id.to_owned()), "operationId ganda: {id}");
            assert!(op["responses"].as_object().is_some_and(|r| !r.is_empty()));
        }
    }
}

#[test]
fn openapi_refuses_a_document_that_fails_semantic_rules() {
    let mut doc = example("orders.json");
    doc["endpoints"][0]["path_params"] = json!({});
    let errors = to_openapi(&doc).unwrap_err();
    assert!(
        errors.iter().any(|e| e.contains("path_params")),
        "{errors:?}"
    );
}

#[test]
fn openapi_endpoint_with_unknown_flow_is_rejected() {
    let doc = example("orders.json");
    let known = flow_ids(&doc);
    assert!(check_handlers(&doc, &known).is_empty());

    let mut without_flow = known.clone();
    let one = known.iter().next().unwrap().clone();
    without_flow.remove(&one);
    let errors = check_handlers(&doc, &without_flow);
    assert!(!errors.is_empty());
    assert!(errors.iter().all(|e| e.contains("tidak ada")));
}
