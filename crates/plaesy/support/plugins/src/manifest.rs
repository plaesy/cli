//! Manifest plugin: nama, versi, ABI, dan daftar capability yang diminta.
//!
//! Manifest diurai dari JSON. Setiap field divalidasi dengan error bertipe. Field yang tidak
//! dikenal ditolak, sehingga kesalahan ketik pada kunci izin tidak diam-diam diabaikan.

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

use crate::abi::{parse_number, Abi};
use crate::capability::Capability;
use crate::error::PluginError;

/// Batas panjang nama plugin.
pub const MAX_NAME_LEN: usize = 64;

/// Batas jumlah capability yang boleh diminta satu manifest.
pub const MAX_CAPABILITIES: usize = 64;

/// Nama plugin yang sudah tervalidasi.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PluginName(String);

impl PluginName {
    /// Memvalidasi nama plugin.
    ///
    /// # Errors
    ///
    /// Mengembalikan `PluginError::InvalidName` jika nama kosong, terlalu panjang, memuat karakter
    /// selain huruf kecil ASCII, angka, dan '-', tidak diawali huruf, atau diakhiri '-'.
    pub fn new(raw: &str) -> Result<Self, PluginError> {
        let bytes = raw.as_bytes();
        let valid = !bytes.is_empty()
            && bytes.len() <= MAX_NAME_LEN
            && bytes[0].is_ascii_lowercase()
            && bytes[bytes.len() - 1] != b'-'
            && bytes
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
        if valid {
            Ok(Self(raw.to_owned()))
        } else {
            Err(PluginError::InvalidName(raw.to_owned()))
        }
    }

    /// Nama sebagai string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Versi semantik `MAJOR.MINOR.PATCH` berupa angka.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl FromStr for Version {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || PluginError::InvalidVersion(s.to_owned());
        let parts: Vec<&str> = s.split('.').collect();
        match parts.as_slice() {
            [major, minor, patch] => {
                match (parse_number(major), parse_number(minor), parse_number(patch)) {
                    (Some(major), Some(minor), Some(patch)) => Ok(Self {
                        major,
                        minor,
                        patch,
                    }),
                    _ => Err(invalid()),
                }
            }
            _ => Err(invalid()),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Manifest plugin yang sudah tervalidasi. Semua capability sudah dinormalisasi dan unik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: PluginName,
    pub version: Version,
    pub abi: Abi,
    pub capabilities: Vec<Capability>,
}

/// Bentuk mentah JSON sebelum validasi.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    name: String,
    version: String,
    abi: String,
    #[serde(default)]
    capabilities: Vec<Capability>,
}

impl Manifest {
    /// Mengurai dan memvalidasi manifest JSON.
    ///
    /// Contoh bentuk JSON:
    /// `{"name":"notes-sync","version":"0.1.0","abi":"1.0","capabilities":[{"kind":"clock"}]}`.
    ///
    /// # Errors
    ///
    /// Mengembalikan `PluginError::Manifest` jika JSON tidak valid, dan error bertipe lain jika
    /// nama, versi, ABI, atau capability tidak valid.
    pub fn parse(input: &str) -> Result<Self, PluginError> {
        let raw: RawManifest =
            serde_json::from_str(input).map_err(|e| PluginError::Manifest(e.to_string()))?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawManifest) -> Result<Self, PluginError> {
        let name = PluginName::new(&raw.name)?;
        let version = raw.version.parse::<Version>()?;
        let abi = raw.abi.parse::<Abi>()?;

        if raw.capabilities.len() > MAX_CAPABILITIES {
            return Err(PluginError::InvalidCapability(format!(
                "{} melebihi batas {MAX_CAPABILITIES}",
                raw.capabilities.len()
            )));
        }

        let mut seen = HashSet::new();
        let mut capabilities = Vec::with_capacity(raw.capabilities.len());
        for capability in raw.capabilities {
            let capability = capability.normalize()?;
            if !seen.insert(capability.clone()) {
                return Err(PluginError::DuplicateCapability(capability.to_string()));
            }
            capabilities.push(capability);
        }

        Ok(Self {
            name,
            version,
            abi,
            capabilities,
        })
    }
}

#[cfg(test)]
#[path = "manifest_test.rs"]
mod manifest_test;
