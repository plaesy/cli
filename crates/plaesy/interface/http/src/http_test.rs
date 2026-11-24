use super::*;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use plaesy_adapter_sqlite::SqliteStore;
use tower::ServiceExt;

fn app() -> (tempfile::TempDir, Router) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("docs.sqlite")).unwrap();
    let shared: SharedStore = Arc::new(store);
    (dir, router(shared))
}

async fn send(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(match body {
            Some(value) => Body::from(value.to_string()),
            None => Body::empty(),
        })
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}

#[tokio::test]
async fn http_health_is_ok() {
    let (_dir, app) = app();
    let (status, body) = send(&app, "GET", "/v1/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "ok": true }));
}

#[tokio::test]
async fn http_put_get_list_delete_roundtrip_with_slash_in_kind() {
    let (_dir, app) = app();
    let doc = json!({ "ir": "flow-ir/1", "name": "checkout" });
    let (status, _) = send(
        &app,
        "PUT",
        "/v1/documents/flow-ir%2F1/checkout",
        Some(doc.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(&app, "GET", "/v1/documents/flow-ir%2F1/checkout", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, doc);

    let (status, body) = send(&app, "GET", "/v1/documents/flow-ir%2F1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "kind": "flow-ir/1", "ids": ["checkout"] }));

    let (status, body) = send(&app, "DELETE", "/v1/documents/flow-ir%2F1/checkout", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "deleted": true }));

    let (status, _) = send(&app, "GET", "/v1/documents/flow-ir%2F1/checkout", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_control_character_in_id_is_bad_request() {
    let (_dir, app) = app();
    // %0A adalah newline: karakter kontrol ditolak oleh validasi kunci.
    let (status, body) = send(
        &app,
        "PUT",
        "/v1/documents/flow-ir%2F1/a%0Ab",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("kunci tidak valid"));
}

fn example(folder: &str, name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../docs/adr/examples")
        .join(folder)
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

#[tokio::test]
async fn http_validate_reports_valid_and_errors() {
    let (_dir, app) = app();
    let doc = example("data-ir-1", "crm.json");
    let (status, body) = send(&app, "POST", "/v1/validate", Some(doc.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ir"], "data-ir/1");
    assert_eq!(body["valid"], true);

    let mut broken = doc;
    broken["tables"][0]["primary_key"] = json!(["tidak_ada"]);
    let (status, body) = send(&app, "POST", "/v1/validate", Some(broken)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["valid"], false);
    assert!(!body["errors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn http_openapi_returns_spec_or_422() {
    let (_dir, app) = app();
    let (status, body) = send(
        &app,
        "POST",
        "/v1/openapi",
        Some(example("api-ir-1", "orders.json")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["openapi"], "3.1.0");

    let mut broken = example("api-ir-1", "orders.json");
    broken["endpoints"][0]["path_params"] = json!({});
    let (status, body) = send(&app, "POST", "/v1/openapi", Some(broken)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["errors"].is_array());
}

#[tokio::test]
async fn http_codegen_flow_js_and_go_return_source_and_report() {
    let (_dir, app) = app();
    let flow = example("flow-ir-1", "kirim-faktur.json");
    let (status, body) = send(&app, "POST", "/v1/codegen/flow/js", Some(flow.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["source"]
        .as_str()
        .unwrap()
        .contains("export async function"));
    assert!(body["report"]["mapped"].is_array());

    let (status, body) = send(&app, "POST", "/v1/codegen/flow/go", Some(flow)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["source"].as_str().unwrap().contains("package flows"));
}

#[tokio::test]
async fn http_codegen_flow_invalid_or_unknown_language_is_refused() {
    let (_dir, app) = app();
    let mut flow = example("flow-ir-1", "kirim-faktur.json");
    flow["modules"] = json!([]);
    let (status, _) = send(&app, "POST", "/v1/codegen/flow/js", Some(flow)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = send(
        &app,
        "POST",
        "/v1/codegen/flow/rust",
        Some(example("flow-ir-1", "kirim-faktur.json")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
