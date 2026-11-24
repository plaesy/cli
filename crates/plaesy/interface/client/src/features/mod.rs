//! Fitur klien (ADR 0011). Setiap fitur berisi `model` (view tanpa I/O), `state` (reducer),
//! dan `services` (membangun request lewat trait `ApiClient` milik fitur ini).

pub mod data;
pub mod flow;

/// Permintaan HTTP yang dibangun services. Tanpa tipe JSON agar crate ini tidak perlu dependensi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiRequest {
    pub method: &'static str,
    pub path: String,
    pub body: Option<String>,
}

/// Pelaksana request. Implementasinya ada di adapter, bukan di folder ini.
pub trait ApiClient {
    fn send(&self, request: ApiRequest) -> Result<String, String>;
}
