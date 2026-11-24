//! Batas sumber daya eksekusi plugin sebagai nilai konfigurasi host.
//!
//! Nilai ini tidak diambil dari manifest. Pengguna atau host yang menentukan batasnya. Satuan
//! memori dibuat selaras dengan halaman WebAssembly (64 KiB) agar siap dipetakan ke runtime.

use serde::{Deserialize, Serialize};

use crate::error::PluginError;

/// Ukuran satu halaman memori WebAssembly.
pub const WASM_PAGE_BYTES: u64 = 65_536;
/// Batas bawah memori: 16 halaman (1 MiB).
pub const MIN_MEMORY_BYTES: u64 = 16 * WASM_PAGE_BYTES;
/// Batas atas memori: 4 GiB, batas alamat wasm32.
pub const MAX_MEMORY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Batas atas fuel per pemanggilan.
pub const MAX_FUEL: u64 = 1_000_000_000_000;
/// Default fuel: 10 juta unit.
pub const DEFAULT_FUEL: u64 = 10_000_000;
/// Default memori: 64 MiB.
pub const DEFAULT_MEMORY_BYTES: u64 = 64 * 1024 * 1024;

/// Batas fuel dan memori untuk satu instance plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLimits {
    /// Unit komputasi yang boleh dipakai, dalam satuan fuel runtime.
    pub fuel: u64,
    /// Memori linear maksimum dalam byte. Harus kelipatan `WASM_PAGE_BYTES`.
    pub memory_bytes: u64,
}

impl ResourceLimits {
    /// Membuat batas dan langsung memvalidasinya.
    ///
    /// # Errors
    ///
    /// Mengembalikan `PluginError::InvalidLimits` jika nilai di luar rentang yang diizinkan.
    pub fn new(fuel: u64, memory_bytes: u64) -> Result<Self, PluginError> {
        let limits = Self {
            fuel,
            memory_bytes,
        };
        limits.validate()?;
        Ok(limits)
    }

    /// Memeriksa rentang fuel dan memori, serta kelipatan halaman.
    ///
    /// # Errors
    ///
    /// Mengembalikan `PluginError::InvalidLimits` jika fuel nol atau melebihi `MAX_FUEL`, atau
    /// memori di luar rentang, atau memori bukan kelipatan `WASM_PAGE_BYTES`.
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.fuel == 0 || self.fuel > MAX_FUEL {
            return Err(PluginError::InvalidLimits(format!(
                "fuel {} di luar rentang 1..={MAX_FUEL}",
                self.fuel
            )));
        }
        if self.memory_bytes < MIN_MEMORY_BYTES || self.memory_bytes > MAX_MEMORY_BYTES {
            return Err(PluginError::InvalidLimits(format!(
                "memori {} byte di luar rentang {MIN_MEMORY_BYTES}..={MAX_MEMORY_BYTES}",
                self.memory_bytes
            )));
        }
        if !self.memory_bytes.is_multiple_of(WASM_PAGE_BYTES) {
            return Err(PluginError::InvalidLimits(format!(
                "memori {} byte bukan kelipatan {WASM_PAGE_BYTES} (satu halaman wasm)",
                self.memory_bytes
            )));
        }
        Ok(())
    }

    /// Jumlah halaman wasm yang setara dengan `memory_bytes`.
    #[must_use]
    pub const fn memory_pages(&self) -> u64 {
        self.memory_bytes / WASM_PAGE_BYTES
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            fuel: DEFAULT_FUEL,
            memory_bytes: DEFAULT_MEMORY_BYTES,
        }
    }
}

#[cfg(test)]
#[path = "limits_test.rs"]
mod limits_test;
