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
