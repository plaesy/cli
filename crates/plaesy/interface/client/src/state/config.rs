//! Pengaturan klien yang bukan rahasia: alamat server, project, dan nama perangkat. Token tidak
//! disimpan di sini; token ada di penyimpanan rahasia OS. Murni: membaca dan menulis teks, tanpa
//! berkas.

use crate::adapter::http::contract::valid_project_id;
use serde_json::{json, Value};

/// Pengaturan yang disimpan di berkas konfigurasi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSettings {
    /// Base URL tanpa `/` di akhir, mis. `https://plaesy.example.com`.
    pub base_url: String,
    /// Id project, berbentuk `prj_...`.
    pub project: String,
    /// Nama perangkat yang dikirim saat pairing.
    pub device_name: String,
}

impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8790".to_owned(),
            project: String::new(),
            device_name: "perangkat-desktop".to_owned(),
        }
    }
}

/// Membaca pengaturan dari JSON. Field yang hilang memakai nilai bawaan.
///
/// # Errors
///
/// Bila teks bukan JSON objek.
pub fn parse_settings(text: &str) -> Result<ClientSettings, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| format!("pengaturan bukan JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "pengaturan harus berupa objek JSON".to_owned())?;
    let defaults = ClientSettings::default();
    let field = |key: &str, fallback: String| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map_or(fallback, str::to_owned)
    };
    Ok(ClientSettings {
        base_url: field("base_url", defaults.base_url),
        project: field("project", defaults.project),
        device_name: field("device_name", defaults.device_name),
    })
}

/// Teks JSON untuk pengaturan. Tidak memuat token.
#[must_use]
pub fn format_settings(settings: &ClientSettings) -> String {
    json!({
        "base_url": settings.base_url,
        "project": settings.project,
        "device_name": settings.device_name,
    })
    .to_string()
}

/// Memeriksa pengaturan sebelum dipakai: alamat harus `http://` atau `https://`, project harus
/// `prj_` lalu huruf kecil atau angka, dan nama perangkat tidak boleh kosong.
///
/// # Errors
///
/// Pesan pertama yang tidak lolos pemeriksaan.
pub fn validate_settings(settings: &ClientSettings) -> Result<(), String> {
    let base = settings.base_url.trim();
    if !(base.starts_with("http://") || base.starts_with("https://")) {
        return Err("alamat server harus diawali http:// atau https://".to_owned());
    }
    if !valid_project_id(&settings.project) {
        return Err("project harus berbentuk prj_ lalu huruf kecil atau angka".to_owned());
    }
    if settings.device_name.trim().is_empty() {
        return Err("nama perangkat tidak boleh kosong".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;
