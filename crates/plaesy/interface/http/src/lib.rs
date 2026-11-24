//! Antarmuka HTTP untuk dokumen (ADR 0003: interface/http). Hanya memanggil port
//! `DocumentStore`; tidak mengenal SQLite atau berkas.
//!
//! Rute:
//! - `GET /v1/health`
//! - `GET /v1/documents/{kind}` daftar id untuk satu kind
//! - `GET|PUT|DELETE /v1/documents/{kind}/{id}`
//!
//! `kind` dikirim sebagai satu segmen dengan `/` di-encode (`flow-ir%2F1`).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use plaesy_service::port::{DocumentStore, StoreError};
use serde_json::{json, Value};
use std::sync::Arc;

/// Penyimpanan yang dipakai router. `Arc` agar bisa dibagikan ke setiap permintaan.
pub type SharedStore = Arc<dyn DocumentStore + Send + Sync>;

/// Membangun router untuk `store`.
pub fn router(store: SharedStore) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/documents/{kind}", get(list_documents))
        .route(
            "/v1/documents/{kind}/{id}",
            get(get_document).put(put_document).delete(delete_document),
        )
        .route("/v1/validate", post(validate_ir))
        .route("/v1/openapi", post(openapi))
        .route("/v1/codegen/flow/{lang}", post(codegen_flow))
        .with_state(store)
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn list_documents(State(store): State<SharedStore>, Path(kind): Path<String>) -> Response {
    match store.list(&kind) {
        Ok(ids) => Json(json!({ "kind": kind, "ids": ids })).into_response(),
        Err(error) => error_response(&error),
    }
}

async fn get_document(
    State(store): State<SharedStore>,
    Path((kind, id)): Path<(String, String)>,
) -> Response {
    match store.get(&kind, &id) {
        Ok(Some(document)) => Json(document).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "tidak ditemukan" })),
        )
            .into_response(),
        Err(error) => error_response(&error),
    }
}

async fn put_document(
    State(store): State<SharedStore>,
    Path((kind, id)): Path<(String, String)>,
    Json(document): Json<Value>,
) -> Response {
    match store.put(&kind, &id, &document) {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Err(error) => error_response(&error),
    }
}

async fn delete_document(
    State(store): State<SharedStore>,
    Path((kind, id)): Path<(String, String)>,
) -> Response {
    match store.delete(&kind, &id) {
        Ok(existed) => Json(json!({ "deleted": existed })).into_response(),
        Err(error) => error_response(&error),
    }
}

async fn validate_ir(Json(doc): Json<Value>) -> Response {
    let report = plaesy_service::validate::validate_document(&doc);
    let valid = report.errors.is_empty();
    let errors: Vec<String> = report.errors.iter().map(ToString::to_string).collect();
    Json(json!({ "ir": report.ir, "valid": valid, "errors": errors })).into_response()
}

async fn openapi(Json(doc): Json<Value>) -> Response {
    match plaesy_domain_api::openapi::to_openapi(&doc) {
        Ok(spec) => Json(spec).into_response(),
        Err(errors) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "errors": errors })),
        )
            .into_response(),
    }
}

async fn codegen_flow(Path(lang): Path<String>, Json(doc): Json<Value>) -> Response {
    // Setiap bahasa punya tipe Report sendiri, jadi laporan diubah ke JSON per arm.
    macro_rules! report_json {
        ($report:expr) => {
            json!({
                "mapped": $report.mapped.iter().map(|m| json!({ "module": m.module, "kind": m.kind })).collect::<Vec<_>>(),
                "lost": $report.lost.iter().map(|l| json!({ "module": l.module, "kind": l.kind, "reason": l.reason })).collect::<Vec<_>>(),
            })
        };
    }
    let generated: Result<(String, Value), String> = match lang.as_str() {
        "js" => plaesy_adapter_codegen_js::generate(&doc)
            .map(|g| (g.source, report_json!(g.report)))
            .map_err(|error| error.to_string()),
        "go" => plaesy_adapter_codegen_go::generate(&doc)
            .map(|g| (g.source, report_json!(g.report)))
            .map_err(|error| error.to_string()),
        other => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("bahasa {other:?} tidak didukung") })),
            )
                .into_response();
        }
    };
    match generated {
        Ok((source, report)) => Json(json!({ "source": source, "report": report })).into_response(),
        Err(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "errors": [error] })),
        )
            .into_response(),
    }
}

/// Memetakan galat port ke status HTTP. Kunci tidak valid menjadi 400, sisanya 500.
fn error_response(error: &StoreError) -> Response {
    let status = match error {
        StoreError::InvalidKey(_) => StatusCode::BAD_REQUEST,
        StoreError::Backend(_) | StoreError::UnsupportedSchema { .. } => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (status, Json(json!({ "error": error.to_string() }))).into_response()
}

#[cfg(test)]
#[path = "http_test.rs"]
mod http_test;
