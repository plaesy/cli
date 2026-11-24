//! Port klien HTTP untuk langkah flow yang memanggil HTTP (ADR 0003: adapter/http-client).
//! Implementasi nyata ada di `plaesy-adapter-http-client`.

use std::fmt;
use std::time::Duration;

/// Metode HTTP yang didukung port ini.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

/// Satu permintaan HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub timeout: Duration,
}

/// Balasan HTTP. Status non-2xx tetap dikembalikan sebagai balasan, bukan galat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// Galat transport: koneksi, timeout, atau URL tidak valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpError(pub String);

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "http: {}", self.0)
    }
}

impl std::error::Error for HttpError {}

/// Klien HTTP yang dipakai flow.
pub trait HttpClient {
    /// Mengirim permintaan dan menunggu balasan.
    ///
    /// # Errors
    ///
    /// Galat transport. Status HTTP tidak dianggap galat.
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;
}
