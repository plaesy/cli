//! Kontrak klien ke backend (cli-137): konfigurasi, path tenant, If-Match, dan pemetaan galat.
//!
//! Murni: tanpa I/O. Transport (fetch di web, dan platform lain nanti) memakai nilai di sini.
//! Nama rute di sini bersifat provisional sampai cli-137 mengonfirmasi kontrak final. Base URL,
//! project, dan token adalah konfigurasi, tidak di-hard-code.

use super::encode_segment;
use serde_json::{json, Value};
use std::fmt;

/// Konfigurasi koneksi ke satu backend untuk satu project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Base URL tanpa `/` di akhir, mis. `https://plaesy.example.com`.
    pub base_url: String,
    /// Id project sebagai segmen path, mis. `prj_01...`.
    pub project: String,
    /// Token per perangkat. `None` berarti belum dipasangkan.
    pub token: Option<String>,
}

impl ClientConfig {
    #[must_use]
    pub fn new(base_url: &str, project: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            project: project.to_owned(),
            token: None,
        }
    }

    #[must_use]
    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_owned());
        self
    }

    /// URL lengkap untuk satu dokumen: `{base}/v1/projects/{project}/documents/{kind}/{id}`.
    #[must_use]
    pub fn document_url(&self, kind: &str, id: &str) -> String {
        format!(
            "{}{}",
            self.base_url,
            document_path(&self.project, kind, id)
        )
    }

    /// Header auth. Kosong bila belum ada token.
    #[must_use]
    pub fn headers(&self) -> Vec<(String, String)> {
        match &self.token {
            Some(token) => vec![("Authorization".to_owned(), format!("Bearer {token}"))],
            None => Vec::new(),
        }
    }
}

/// Path tenant dokumen. `/` di `kind` menjadi `%2F`, sama dengan path lama.
#[must_use]
pub fn document_path(project: &str, kind: &str, id: &str) -> String {
    format!(
        "/v1/projects/{}/documents/{}/{}",
        encode_segment(project),
        encode_segment(kind),
        encode_segment(id)
    )
}

/// Nilai If-Match dari ETag. ETag dibungkus tanda kutip bila belum dikutip.
#[must_use]
pub fn if_match_value(etag: &str) -> String {
    if etag.starts_with('"') {
        etag.to_owned()
    } else {
        format!("\"{etag}\"")
    }
}

/// Path penukaran kode pairing menjadi token (kode `PAIR-...`, sekali pakai, berlaku 5 menit).
pub const PAIRING_EXCHANGE_PATH: &str = "/v1/pairing/exchange";

/// Path rotasi token perangkat. Token lama tetap berlaku 5 menit (grace window).
pub const ROTATE_PATH: &str = "/v1/devices/self/rotate";

/// Body penukaran kode pairing.
#[must_use]
pub fn pairing_body(code: &str) -> Value {
    json!({ "code": code.trim() })
}

/// Token dari balasan pairing atau rotasi. Token berlaku 90 hari.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pairing {
    pub token: String,
    pub expires_at: String,
}

/// Membaca balasan `{"token", "expires_at"}`.
///
/// # Errors
///
/// Bila salah satu field tidak ada atau bukan teks.
pub fn parse_pairing(body: &Value) -> Result<Pairing, String> {
    let field = |key: &str| {
        body.get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("balasan pairing tidak punya `{key}`"))
    };
    Ok(Pairing {
        token: field("token")?,
        expires_at: field("expires_at")?,
    })
}

/// Id project: `prj_` lalu 1 sampai 58 karakter `[a-z0-9]`.
#[must_use]
pub fn valid_project_id(id: &str) -> bool {
    match id.strip_prefix("prj_") {
        Some(rest) => {
            (1..=58).contains(&rest.len())
                && rest
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        }
        None => false,
    }
}

/// Isi 412 `revision_conflict`: ETag terbaru dan dokumen terbaru di server.
#[derive(Debug, Clone, PartialEq)]
pub struct RevisionConflict {
    pub current_etag: String,
    pub document: Value,
}

/// Membaca body 412 `{"error":"revision_conflict","current":{"etag","document"}}`.
#[must_use]
pub fn conflict_from_body(body: &Value) -> Option<RevisionConflict> {
    if body.get("error").and_then(Value::as_str) != Some("revision_conflict") {
        return None;
    }
    let current = body.get("current")?;
    Some(RevisionConflict {
        current_etag: current.get("etag").and_then(Value::as_str)?.to_owned(),
        document: current.get("document")?.clone(),
    })
}

/// Pemetaan 401 dengan kode dari body. `token_expired` dibedakan dari token yang salah.
#[must_use]
pub fn map_unauthorized(error_code: Option<&str>) -> ContractError {
    match error_code {
        Some("token_expired") => ContractError::TokenExpired,
        _ => ContractError::Unauthorized,
    }
}

/// Galat kontrak yang bisa ditampilkan ke pengguna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// 401: token tidak ada, salah, atau sudah dicabut.
    Unauthorized,
    /// 401 dengan `token_expired`: rotasi atau pairing ulang diperlukan.
    TokenExpired,
    /// 403: token sah tetapi tidak punya izin.
    Forbidden,
    /// 412: dokumen sudah berubah sejak dimuat. `current_etag` dari header ETag bila ada.
    Conflict { current_etag: Option<String> },
    /// 428: server mewajibkan If-Match.
    PreconditionRequired,
    /// 413: body lebih dari 1 MiB.
    TooLarge,
    /// 422: IR tidak valid.
    Invalid,
    /// 429: terlalu banyak permintaan. `retry_after_secs` dari header Retry-After bila angka.
    RateLimited { retry_after_secs: Option<u64> },
    /// Status lain yang tidak dipetakan.
    Other { status: u16 },
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => write!(f, "token tidak sah; pasangkan ulang perangkat"),
            Self::TokenExpired => write!(f, "token sudah kedaluwarsa; rotasi atau pasangkan ulang"),
            Self::Forbidden => write!(f, "tidak punya izin untuk tindakan ini"),
            Self::Conflict { .. } => {
                write!(
                    f,
                    "dokumen sudah berubah di server; muat ulang sebelum menyimpan"
                )
            }
            Self::PreconditionRequired => write!(f, "server mewajibkan If-Match"),
            Self::TooLarge => write!(f, "dokumen lebih dari 1 MiB"),
            Self::Invalid => write!(f, "dokumen tidak valid menurut server"),
            Self::RateLimited { retry_after_secs } => match retry_after_secs {
                Some(seconds) => write!(f, "terlalu sering; coba lagi dalam {seconds} detik"),
                None => write!(f, "terlalu sering; coba lagi nanti"),
            },
            Self::Other { status } => write!(f, "server mengembalikan status {status}"),
        }
    }
}

impl std::error::Error for ContractError {}

/// Memetakan status HTTP ke galat kontrak. `None` untuk status 2xx dan 3xx.
#[must_use]
pub fn map_status(
    status: u16,
    etag: Option<&str>,
    retry_after: Option<&str>,
) -> Option<ContractError> {
    match status {
        200..=399 => None,
        401 => Some(ContractError::Unauthorized),
        403 => Some(ContractError::Forbidden),
        412 => Some(ContractError::Conflict {
            current_etag: etag.map(str::to_owned),
        }),
        413 => Some(ContractError::TooLarge),
        422 => Some(ContractError::Invalid),
        428 => Some(ContractError::PreconditionRequired),
        429 => Some(ContractError::RateLimited {
            retry_after_secs: retry_after.and_then(|value| value.trim().parse::<u64>().ok()),
        }),
        other => Some(ContractError::Other { status: other }),
    }
}

#[cfg(test)]
#[path = "contract_test.rs"]
mod contract_test;
