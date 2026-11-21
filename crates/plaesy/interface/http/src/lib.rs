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
    routing::get,
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
