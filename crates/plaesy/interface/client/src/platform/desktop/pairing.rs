//! Pairing di desktop: menukar kode `PAIR-...` menjadi token lewat HTTP native (ureq, sinkron).
//! Aturan status dan body ada di `adapter::http::contract`; file ini hanya membawa bytes.
//!
//! Panggilan ini memblokir thread yang memanggilnya. Batas waktu 15 detik membatasi lamanya.

use crate::adapter::http::contract::{
    classify, pairing_body, parse_pairing, ContractError, Pairing, PAIRING_EXCHANGE_PATH,
};
use serde_json::Value;
use std::time::Duration;

/// Menukar kode pairing menjadi token perangkat.
///
/// # Errors
///
/// Galat jaringan, galat kontrak (401 bila kode salah atau kedaluwarsa), atau balasan yang tidak
/// sesuai kontrak.
pub fn exchange(base_url: &str, code: &str) -> Result<Pairing, ContractError> {
    let url = format!(
        "{}{}",
        base_url.trim_end_matches('/'),
        PAIRING_EXCHANGE_PATH
    );
    let body = pairing_body(code).to_string();
    let reply = ureq::post(&url)
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(15))
        .send_string(&body);
    let (status, etag, retry_after, text) = match reply {
        Ok(response) => parts(response),
        Err(ureq::Error::Status(_, response)) => parts(response),
        Err(ureq::Error::Transport(error)) => {
            return Err(ContractError::Network(error.to_string()));
        }
    };
    if let Some(error) = classify(status, etag.as_deref(), retry_after.as_deref(), &text) {
        return Err(error);
    }
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| ContractError::Network(format!("balasan bukan JSON: {error}")))?;
    parse_pairing(&value).map_err(ContractError::Network)
}

fn parts(response: ureq::Response) -> (u16, Option<String>, Option<String>, String) {
    let status = response.status();
    let etag = response.header("ETag").map(str::to_owned);
    let retry_after = response.header("Retry-After").map(str::to_owned);
    let text = response.into_string().unwrap_or_default();
    (status, etag, retry_after, text)
}
