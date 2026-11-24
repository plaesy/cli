//! Use case penyimpanan dokumen IR (flow, data, api, app, design). Hanya dokumen yang lolos
//! validasi semantik IR-nya sendiri yang disimpan. Dokumen diperiksa ulang saat dimuat, jadi
//! data yang rusak di penyimpanan tidak diam-diam dikembalikan ke pemanggil.

use crate::port::{DocumentStore, StoreError};
use crate::validate::validate_document;
use serde_json::Value;
use std::fmt;

/// Galat use case dokumen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    /// Dokumen melanggar aturan semantik IR-nya, atau IR-nya tidak dikenal.
    Invalid {
        ir: Option<String>,
        errors: Vec<String>,
    },
    /// Dokumen tidak punya `id` berupa string.
    MissingId,
    /// Galat penyimpanan.
    Store(StoreError),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid { ir, errors } => write!(
                f,
                "dokumen {} tidak valid: {}",
                ir.as_deref().unwrap_or("tanpa ir"),
                errors.join("; ")
            ),
            Self::MissingId => write!(f, "dokumen harus punya field id berupa string"),
            Self::Store(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DocumentError {}

impl From<StoreError> for DocumentError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// Menyimpan `doc` setelah validasi. Mengembalikan `id` dokumen.
///
/// # Errors
///
/// [`DocumentError::Invalid`] bila validasi gagal, [`DocumentError::MissingId`], atau galat penyimpanan.
pub fn save(store: &dyn DocumentStore, doc: &Value) -> Result<String, DocumentError> {
    let report = validate_document(doc);
    let Some(ir) = report.ir.clone().filter(|_| report.errors.is_empty()) else {
        return Err(DocumentError::Invalid {
            ir: report.ir,
            errors: report.errors.iter().map(ToString::to_string).collect(),
        });
    };
    let id = doc
        .get("id")
        .and_then(Value::as_str)
        .ok_or(DocumentError::MissingId)?
        .to_owned();
    store.put(&ir, &id, doc)?;
    Ok(id)
}

/// Memuat dokumen `id` dari IR `ir`, lalu memeriksa ulang validasinya.
///
/// # Errors
///
/// Galat penyimpanan, atau [`DocumentError::Invalid`] bila data yang tersimpan tidak lolos.
pub fn load(store: &dyn DocumentStore, ir: &str, id: &str) -> Result<Option<Value>, DocumentError> {
    let Some(doc) = store.get(ir, id)? else {
        return Ok(None);
    };
    let report = validate_document(&doc);
    if !report.errors.is_empty() || report.ir.as_deref() != Some(ir) {
        return Err(DocumentError::Invalid {
            ir: report.ir,
            errors: report.errors.iter().map(ToString::to_string).collect(),
        });
    }
    Ok(Some(doc))
}

/// Daftar id dokumen untuk satu IR.
///
/// # Errors
///
/// Galat penyimpanan.
pub fn list(store: &dyn DocumentStore, ir: &str) -> Result<Vec<String>, DocumentError> {
    Ok(store.list(ir)?)
}

/// Menghapus dokumen. Mengembalikan `true` bila dokumen ada.
///
/// # Errors
///
/// Galat penyimpanan.
pub fn remove(store: &dyn DocumentStore, ir: &str, id: &str) -> Result<bool, DocumentError> {
    Ok(store.delete(ir, id)?)
}
