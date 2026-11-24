use super::*;
use serde_json::json;

#[test]
fn route_matches_literal_segments_and_captures_params() {
    let params = match_route("/customers/:customer_id", "/customers/c-42").unwrap();
    assert_eq!(params.get("customer_id").map(String::as_str), Some("c-42"));
}

#[test]
fn route_rejects_different_literal_or_length() {
    assert!(match_route("/orders", "/customers").is_none());
    assert!(match_route("/customers/:id", "/customers").is_none());
    assert!(match_route("/customers/:id", "/customers/a/b").is_none());
}

#[test]
fn route_root_matches_root_only() {
    assert!(match_route("/", "/").is_some());
    assert!(match_route("/", "/orders").is_none());
}

#[test]
fn route_param_names_lists_capture_segments_in_order() {
    assert_eq!(
        route_param_names("/orders/:order_id/items/:item_id"),
        ["order_id", "item_id"]
    );
    assert!(route_param_names("/orders").is_empty());
}

#[test]
fn screens_lists_declared_params_sorted() {
    let doc = json!({
        "screens": [{
            "id": "scr_a",
            "route": "/a",
            "params": { "zeta": {"type": "string"}, "alpha": {"type": "string"} },
            "root": {"id": "cmp_p", "type": "page"}
        }]
    });
    let list = screens(&doc);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "scr_a");
    assert_eq!(list[0].params, ["alpha", "zeta"]);
}

#[test]
fn find_by_path_returns_the_matching_screen_and_params() {
    let doc = json!({
        "screens": [
            {"id": "scr_orders", "route": "/orders", "params": {}},
            {"id": "scr_customer", "route": "/customers/:customer_id", "params": {"customer_id": {"type": "string"}}}
        ]
    });
    let (screen, params) = find_by_path(&doc, "/customers/c-1").unwrap();
    assert_eq!(screen.id, "scr_customer");
    assert_eq!(params.get("customer_id").map(String::as_str), Some("c-1"));
    assert!(find_by_path(&doc, "/tidak-ada").is_none());
}
