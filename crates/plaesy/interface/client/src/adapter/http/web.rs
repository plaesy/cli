//! Transport HTTP di browser (fitur `web`): `fetch` lewat web-sys. Aturan status dan galat ada di
//! `contract`; file ini hanya memindahkan bytes, status, ETag, dan Retry-After.
//!
//! Dependensi: web-sys, wasm-bindgen, dan wasm-bindgen-futures (semuanya sudah ada di Cargo.lock
//! lewat Dioxus web). Tidak ada penyimpanan di sini: token dan ETag dipegang pemanggil.

use super::contract::{
    classify, if_match_value, pairing_body, parse_pairing, parse_validation, ClientConfig,
    ContractError, Pairing, ValidationReport, PAIRING_EXCHANGE_PATH, VALIDATE_PATH,
};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, Response};

/// Balasan mentah dari server.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub etag: Option<String>,
    pub retry_after: Option<String>,
    pub body: String,
}

/// Dokumen yang dimuat beserta ETag-nya.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedDocument {
    pub etag: Option<String>,
    pub document: Value,
}

async fn send(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> Result<Reply, ContractError> {
    let network = |error: JsValue| {
        ContractError::Network(
            error
                .as_string()
                .unwrap_or_else(|| "galat fetch".to_owned()),
        )
    };
    let header_map = Headers::new().map_err(network)?;
    for (name, value) in headers {
        header_map.set(name, value).map_err(network)?;
    }
    if body.is_some() {
        header_map
            .set("Content-Type", "application/json")
            .map_err(network)?;
    }
    let init = RequestInit::new();
    init.set_method(method);
    init.set_headers(header_map.as_ref());
    if let Some(text) = body {
        init.set_body(&JsValue::from_str(text));
    }
    let request = Request::new_with_str_and_init(url, &init).map_err(network)?;
    let window =
        web_sys::window().ok_or_else(|| ContractError::Network("tidak ada window".to_owned()))?;
    let response: Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(network)?
        .dyn_into()
        .map_err(network)?;
    let etag = response.headers().get("ETag").map_err(network)?;
    let retry_after = response.headers().get("Retry-After").map_err(network)?;
    let text = JsFuture::from(response.text().map_err(network)?)
        .await
        .map_err(network)?;
    Ok(Reply {
        status: response.status(),
        etag,
        retry_after,
        body: text.as_string().unwrap_or_default(),
    })
}

/// Galat kontrak untuk balasan yang bukan 2xx atau 3xx. `None` bila sukses.
fn failure(reply: &Reply) -> Option<ContractError> {
    classify(
        reply.status,
        reply.etag.as_deref(),
        reply.retry_after.as_deref(),
        &reply.body,
    )
}

/// Memvalidasi dokumen IR di server. Token ikut dikirim bila ada.
///
/// # Errors
///
/// Galat jaringan, galat kontrak, atau balasan yang tidak sesuai bentuk validasi.
pub async fn validate_document(
    config: &ClientConfig,
    document: &Value,
) -> Result<ValidationReport, ContractError> {
    let url = format!("{}{}", config.base_url, VALIDATE_PATH);
    let body = document.to_string();
    let reply = send("POST", &url, &config.headers(), Some(&body)).await?;
    if let Some(error) = failure(&reply) {
        return Err(error);
    }
    let value: Value = serde_json::from_str(&reply.body)
        .map_err(|error| ContractError::Network(format!("balasan bukan JSON: {error}")))?;
    parse_validation(&value).map_err(ContractError::Network)
}

/// Menukar kode pairing (`PAIR-...`) menjadi token perangkat.
///
/// # Errors
///
/// Galat jaringan, 401 bila kode salah atau kedaluwarsa, atau balasan yang tidak sesuai kontrak.
pub async fn exchange_pairing(base_url: &str, code: &str) -> Result<Pairing, ContractError> {
    let url = format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        PAIRING_EXCHANGE_PATH
    );
    let body = pairing_body(code).to_string();
    let reply = send("POST", &url, &[], Some(&body)).await?;
    if let Some(error) = failure(&reply) {
        return Err(error);
    }
    let value: Value = serde_json::from_str(&reply.body)
        .map_err(|error| ContractError::Network(format!("balasan bukan JSON: {error}")))?;
    parse_pairing(&value).map_err(ContractError::Network)
}

/// Memuat dokumen dari server beserta ETag-nya.
///
/// # Errors
///
/// Galat kontrak (401, 403, 429, dan lain-lain) atau galat jaringan.
pub async fn load_document(
    config: &ClientConfig,
    kind: &str,
    id: &str,
) -> Result<LoadedDocument, ContractError> {
    let reply = send(
        "GET",
        &config.document_url(kind, id),
        &config.headers(),
        None,
    )
    .await?;
    if let Some(error) = failure(&reply) {
        return Err(error);
    }
    let document: Value = serde_json::from_str(&reply.body)
        .map_err(|error| ContractError::Network(format!("dokumen bukan JSON: {error}")))?;
    Ok(LoadedDocument {
        etag: reply.etag,
        document,
    })
}

/// Menyimpan dokumen dengan If-Match `etag`. Mengembalikan ETag baru bila server mengirimnya.
///
/// # Errors
///
/// 412 bila dokumen berubah sejak dimuat (`Conflict`), 428 bila If-Match hilang, dan galat lain.
pub async fn save_document(
    config: &ClientConfig,
    kind: &str,
    id: &str,
    document: &Value,
    etag: Option<&str>,
) -> Result<Option<String>, ContractError> {
    let mut headers = config.headers();
    if let Some(etag) = etag {
        headers.push(("If-Match".to_owned(), if_match_value(etag)));
    }
    let body = document.to_string();
    let reply = send("PUT", &config.document_url(kind, id), &headers, Some(&body)).await?;
    if let Some(error) = failure(&reply) {
        return Err(error);
    }
    Ok(reply.etag)
}
