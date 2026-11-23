//! Error bertipe untuk model plugin.

use thiserror::Error;

/// Kegagalan saat memvalidasi manifest, ABI, capability, atau batas sumber daya.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginError {
    /// JSON manifest tidak bisa diurai atau memuat field yang tidak dikenal.
    #[error("manifest tidak valid: {0}")]
    Manifest(String),
    #[error("nama plugin {0:?} tidak valid: 1 sampai 64 karakter huruf kecil, angka, dan '-', diawali huruf, tidak diakhiri '-'")]
    InvalidName(String),
    #[error("versi {0:?} tidak valid: harus MAJOR.MINOR.PATCH berupa angka")]
    InvalidVersion(String),
    #[error("ABI {0:?} tidak valid: harus MAJOR.MINOR berupa angka")]
    InvalidAbi(String),
    #[error("ABI plugin {plugin} tidak didukung host ABI {host}")]
    UnsupportedAbi { plugin: String, host: String },
    #[error("capability tidak valid: {0}")]
    InvalidCapability(String),
    #[error("capability duplikat: {0}")]
    DuplicateCapability(String),
    #[error("batas sumber daya tidak valid: {0}")]
    InvalidLimits(String),
    #[error("runtime gagal: {0}")]
    Runtime(String),
}
