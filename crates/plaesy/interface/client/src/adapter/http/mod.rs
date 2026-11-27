//! Klien HTTP murni untuk rute `plaesy-interface-http`. Membangun [`Request`] dan
//! menerjemahkan [`Response`]. Pengiriman sebenarnya dilakukan [`Transport`] milik platform.

use serde_json::Value;
use std::fmt;

/// Satu permintaan HTTP yang akan dikirim.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Value>,
}

/// Balasan mentah dari transport.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: Value,
}

/// Pengiriman permintaan. Platform menyediakan implementasinya.
pub trait Transport {
    /// Mengirim `request` dan mengembalikan balasan, atau galat transport.
    ///
    /// # Errors
    ///
    /// Galat koneksi atau pembacaan respons.
    fn send(&mut self, request: &Request) -> Result<Response, String>;
}

/// Galat klien yang bisa ditampilkan ke pengguna.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientError {
    NotFound,
    Invalid(Value),
    Server { status: u16, body: Value },
    Transport(String),
    UnknownLanguage(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "tidak ditemukan"),
            Self::Invalid(body) => write!(f, "dokumen tidak valid: {body}"),
            Self::Server { status, body } => write!(f, "server {status}: {body}"),
            Self::Transport(message) => write!(f, "transport: {message}"),
            Self::UnknownLanguage(lang) => write!(f, "bahasa codegen tidak dikenal: {lang}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Mengenkode segmen path. `/` di `kind` menjadi `%2F`, karakter lain yang bukan unreserved
/// juga di-percent-encode.
#[must_use]
pub fn encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Path dokumen: `/v1/documents/{kind}/{id}`.
#[must_use]
pub fn document_path(kind: &str, id: &str) -> String {
    format!(
        "/v1/documents/{}/{}",
        encode_segment(kind),
        encode_segment(id)
    )
}

#[must_use]
pub fn get_document(kind: &str, id: &str) -> Request {
    Request {
        method: "GET",
        path: document_path(kind, id),
        body: None,
    }
}

#[must_use]
pub fn put_document(kind: &str, id: &str, document: Value) -> Request {
    Request {
        method: "PUT",
        path: document_path(kind, id),
        body: Some(document),
    }
}

#[must_use]
pub fn delete_document(kind: &str, id: &str) -> Request {
    Request {
        method: "DELETE",
        path: document_path(kind, id),
        body: None,
    }
}

#[must_use]
pub fn list_documents(kind: &str) -> Request {
    Request {
        method: "GET",
        path: format!("/v1/documents/{}", encode_segment(kind)),
        body: None,
    }
}

#[must_use]
pub fn validate(document: Value) -> Request {
    Request {
        method: "POST",
        path: "/v1/validate".to_owned(),
        body: Some(document),
    }
}

#[must_use]
pub fn openapi(document: Value) -> Request {
    Request {
        method: "POST",
        path: "/v1/openapi".to_owned(),
        body: Some(document),
    }
}

/// Permintaan codegen. Bahasa hanya `js` atau `go`.
///
/// # Errors
///
/// [`ClientError::UnknownLanguage`] untuk bahasa lain.
pub fn codegen(lang: &str, document: Value) -> Result<Request, ClientError> {
    match lang {
        "js" | "go" => Ok(Request {
            method: "POST",
            path: format!("/v1/codegen/flow/{lang}"),
            body: Some(document),
        }),
        other => Err(ClientError::UnknownLanguage(other.to_owned())),
    }
}

/// Menerjemahkan balasan menjadi nilai JSON, atau galat yang sesuai.
///
/// # Errors
///
/// Status 404, 422, atau status lain di luar 2xx.
pub fn interpret(response: Response) -> Result<Value, ClientError> {
    match response.status {
        200..=299 => Ok(response.body),
        404 => Err(ClientError::NotFound),
        422 => Err(ClientError::Invalid(response.body)),
        status => Err(ClientError::Server {
            status,
            body: response.body,
        }),
    }
}

/// Mengirim `request` lewat `transport`, lalu menerjemahkan balasannya.
///
/// # Errors
///
/// Galat transport atau galat [`interpret`].
pub fn execute(transport: &mut dyn Transport, request: &Request) -> Result<Value, ClientError> {
    let response = transport.send(request).map_err(ClientError::Transport)?;
    interpret(response)
}

#[cfg(test)]
#[path = "http_test.rs"]
mod http_test;

pub mod contract;
