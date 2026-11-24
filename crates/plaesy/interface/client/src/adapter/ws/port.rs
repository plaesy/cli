//! Port transport realtime. Tidak memakai tipe platform: setiap platform (web, desktop, mobile)
//! membungkus socket-nya sendiri di balik trait ini.

use serde_json::Value;

/// Koneksi realtime dua arah yang memakai pesan JSON.
pub trait Transport {
    /// Membuka koneksi dan mengirim pesan hello pertama.
    ///
    /// # Errors
    ///
    /// Galat koneksi.
    fn connect(&mut self, hello: &Value) -> Result<(), String>;

    /// Menerima pesan berikutnya. `Ok(None)` berarti koneksi ditutup.
    ///
    /// # Errors
    ///
    /// Galat pembacaan.
    fn recv(&mut self) -> Result<Option<Value>, String>;

    /// Menutup koneksi.
    fn close(&mut self);
}
