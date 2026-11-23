//! Rencana muat plugin dan antarmuka runtime.
//!
//! `plan_load` murni: manifest, ABI, persetujuan, dan batas sumber daya diperiksa tanpa I/O.
//! Runtime WebAssembly (Wasmtime) belum ada di crate ini. Ia akan mengimplementasikan
//! `PluginRuntime` dan menerima `LoadPlan` yang sudah tervalidasi.

use crate::abi::{check_compatible, Abi, HOST_ABI};
use crate::capability::{self, Capability, Grant};
use crate::error::PluginError;
use crate::limits::ResourceLimits;
use crate::manifest::Manifest;

/// Konfigurasi host yang berlaku untuk setiap plugin yang dimuat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostConfig {
    pub abi: Abi,
    pub limits: ResourceLimits,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            abi: HOST_ABI,
            limits: ResourceLimits::default(),
        }
    }
}

/// Hasil pemeriksaan yang siap diberikan ke runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadPlan {
    pub manifest: Manifest,
    pub grant: Grant,
    pub limits: ResourceLimits,
}

/// Memeriksa manifest dan menghitung capability yang benar-benar diberikan.
///
/// Urutannya: batas sumber daya, manifest, kompatibilitas ABI, normalisasi persetujuan, lalu
/// irisan capability. Plugin tetap bisa dimuat dengan irisan yang lebih sempit. Capability yang
/// ditahan muncul di `grant.denied`, dan runtime wajib menolak akses ke sana.
///
/// # Errors
///
/// Mengembalikan error bertipe dari setiap tahap yang gagal.
pub fn plan_load(
    manifest_json: &str,
    approved: &[Capability],
    host: &HostConfig,
) -> Result<LoadPlan, PluginError> {
    host.limits.validate()?;
    let manifest = Manifest::parse(manifest_json)?;
    check_compatible(manifest.abi, host.abi)?;
    let approved = approved
        .iter()
        .cloned()
        .map(Capability::normalize)
        .collect::<Result<Vec<_>, _>>()?;
    let granted = capability::grant(&manifest.capabilities, &approved);
    Ok(LoadPlan {
        manifest,
        grant: granted,
        limits: host.limits,
    })
}

/// Seam untuk runtime eksekusi. Implementasi Wasmtime akan menaruh logika fuel dan memori di sini.
pub trait PluginRuntime {
    /// Handle instance yang sudah berjalan atau siap dipanggil.
    type Instance;

    /// Membuat instance dari modul biner sesuai `plan`.
    ///
    /// # Errors
    ///
    /// Mengembalikan error bertipe bila modul tidak bisa dikompilasi atau diinstansiasi.
    fn instantiate(&self, plan: &LoadPlan, module: &[u8]) -> Result<Self::Instance, PluginError>;
}

#[cfg(test)]
#[path = "runtime_test.rs"]
mod runtime_test;
