use super::*;
use serde_json::json;

fn doc_with_targets(success_screen: &str, success_params: Value) -> Value {
    json!({
        "start_screen": "scr_a",
        "screens": [
            {
                "id": "scr_a",
                "route": "/a",
                "params": {},
                "root": {
                    "id": "cmp_root",
                    "type": "page",
                    "children": [{
                        "id": "cmp_btn",
                        "type": "button",
                        "events": [{
                            "on": "click",
                            "action": {
                                "flow_id": "flw_x",
                                "inputs": {},
                                "on_success": { "screen": success_screen, "params": success_params }
                            }
                        }]
                    }]
                }
            },
            {
                "id": "scr_b",
                "route": "/b/:order_id",
                "params": { "order_id": { "type": "string" } },
                "root": { "id": "cmp_b", "type": "page" }
            },
            {
                "id": "scr_c",
                "route": "/c",
                "params": {},
                "root": { "id": "cmp_c", "type": "page" }
            }
        ]
    })
}

#[test]
fn navigation_collects_targets_from_nested_components() {
    let doc = doc_with_targets("scr_b", json!({ "order_id": {"param": "x"} }));
    let targets = navigation_targets(&doc);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].from, "scr_a");
    assert_eq!(targets[0].to, "scr_b");
    assert!(targets[0].params.contains("order_id"));
}

#[test]
fn navigation_valid_target_passes() {
    let doc = doc_with_targets("scr_b", json!({ "order_id": {"flow_output": "id"} }));
    assert!(validate_navigation(&doc).is_empty());
}

#[test]
fn navigation_to_missing_screen_is_an_error() {
    let doc = doc_with_targets("scr_hilang", json!({}));
    let errors = validate_navigation(&doc);
    assert!(
        errors.iter().any(|e| e.contains("scr_hilang")),
        "{errors:?}"
    );
}

#[test]
fn navigation_undeclared_param_is_an_error() {
    let doc = doc_with_targets("scr_b", json!({ "order_id": {}, "tidak_ada": {} }));
    let errors = validate_navigation(&doc);
    assert!(errors.iter().any(|e| e.contains("tidak_ada")), "{errors:?}");
}

#[test]
fn navigation_missing_route_param_is_an_error() {
    let doc = doc_with_targets("scr_b", json!({}));
    let errors = validate_navigation(&doc);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("order_id") && e.contains("tidak dikirim")),
        "{errors:?}"
    );
}

#[test]
fn navigation_reachable_and_unreachable_from_start() {
    let doc = doc_with_targets("scr_b", json!({ "order_id": {} }));
    let reach = reachable(&doc);
    assert!(reach.contains("scr_a"));
    assert!(reach.contains("scr_b"));
    assert!(!reach.contains("scr_c"));
    assert_eq!(unreachable(&doc), ["scr_c"]);
}

#[test]
fn navigation_missing_start_screen_is_reported() {
    let mut doc = doc_with_targets("scr_b", json!({ "order_id": {} }));
    doc["start_screen"] = json!("scr_hilang");
    assert!(validate_navigation(&doc)
        .iter()
        .any(|e| e.contains("start_screen")));
}
