//! Port penyimpanan dokumen. Adapter (misalnya `plaesy-adapter-sqlite`) mengimplementasikannya.
//!
//! Dokumen disimpan sebagai JSON per `kind` (nama IR atau konteks, misalnya `flow-ir/1`) dan
//! `id`. Port ini tidak tahu tentang SQL atau berkas; itu urusan adapter.

use serde_json::Value;
use std::fmt;

/// Galat dari port penyimpanan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// `kind` atau `id` kosong, atau mengandung karakter kontrol.
    InvalidKey(String),
    /// Dokumen tidak bisa dibaca atau ditulis karena alasan penyimpanan.
    Backend(String),
    /// Skema penyimpanan lebih baru dari yang didukung adapter.
    UnsupportedSchema { found: i64, supported: i64 },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(message) => write!(f, "kunci tidak valid: {message}"),
            Self::Backend(message) => write!(f, "penyimpanan gagal: {message}"),
            Self::UnsupportedSchema { found, supported } => write!(
                f,
                "skema penyimpanan {found} lebih baru dari yang didukung ({supported})"
            ),
        }
    }
}

impl std::error::Error for StoreError {}

/// Penyimpanan dokumen JSON yang dikunci oleh `(kind, id)`.
pub trait DocumentStore {
    /// Menyimpan atau menimpa dokumen.
    ///
    /// # Errors
    ///
    /// Kunci tidak valid, atau penyimpanan gagal.
    fn put(&self, kind: &str, id: &str, document: &Value) -> Result<(), StoreError>;

    /// Mengambil dokumen. `Ok(None)` berarti tidak ada.
    ///
    /// # Errors
    ///
    /// Kunci tidak valid, atau penyimpanan gagal.
    fn get(&self, kind: &str, id: &str) -> Result<Option<Value>, StoreError>;

    /// Daftar id untuk `kind`, terurut naik.
    ///
    /// # Errors
    ///
    /// `kind` tidak valid, atau penyimpanan gagal.
    fn list(&self, kind: &str) -> Result<Vec<String>, StoreError>;

    /// Menghapus dokumen. Mengembalikan `true` bila dokumen ada sebelumnya.
    ///
    /// # Errors
    ///
    /// Kunci tidak valid, atau penyimpanan gagal.
    fn delete(&self, kind: &str, id: &str) -> Result<bool, StoreError>;
}

/// Memeriksa `kind` atau `id`: tidak kosong, panjang maksimal 256, dan tanpa karakter kontrol.
///
/// # Errors
///
/// [`StoreError::InvalidKey`] bila aturan dilanggar.
pub fn check_key(label: &str, key: &str) -> Result<(), StoreError> {
    if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
        return Err(StoreError::InvalidKey(format!(
            "{label} harus 1 sampai 256 karakter tanpa kontrol"
        )));
    }
    Ok(())
}
