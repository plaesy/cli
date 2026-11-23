//! Versi ABI plugin dan pemeriksaan kompatibilitas dengan host.

use std::fmt;
use std::str::FromStr;

use crate::error::PluginError;

/// ABI host yang dipakai saat ini. Naikkan minor untuk perubahan yang kompatibel ke belakang.
pub const HOST_ABI: Abi = Abi::new(1, 0);

/// Versi ABI berformat `MAJOR.MINOR`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Abi {
    major: u32,
    minor: u32,
}

impl Abi {
    /// Membuat ABI dari komponen numeriknya.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Nomor major. Perubahan major memutus kompatibilitas.
    #[must_use]
    pub const fn major(self) -> u32 {
        self.major
    }

    /// Nomor minor. Plugin boleh meminta minor yang sama atau lebih rendah dari host.
    #[must_use]
    pub const fn minor(self) -> u32 {
        self.minor
    }
}

impl FromStr for Abi {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || PluginError::InvalidAbi(s.to_owned());
        let parts: Vec<&str> = s.split('.').collect();
        match parts.as_slice() {
            [major, minor] => match (parse_number(major), parse_number(minor)) {
                (Some(major), Some(minor)) => Ok(Self::new(major, minor)),
                _ => Err(invalid()),
            },
            _ => Err(invalid()),
        }
    }
}

impl fmt::Display for Abi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Memeriksa bahwa plugin dengan ABI `plugin` bisa dimuat host dengan ABI `host`.
///
/// Syaratnya: major sama, dan minor plugin tidak lebih tinggi dari minor host.
///
/// # Errors
///
/// Mengembalikan `PluginError::UnsupportedAbi` jika salah satu syarat tidak terpenuhi.
pub fn check_compatible(plugin: Abi, host: Abi) -> Result<(), PluginError> {
    if plugin.major == host.major && plugin.minor <= host.minor {
        Ok(())
    } else {
        Err(PluginError::UnsupportedAbi {
            plugin: plugin.to_string(),
            host: host.to_string(),
        })
    }
}

/// Mengurai angka desimal tanpa tanda. Menolak `+1`, `-1`, dan string kosong.
pub(crate) fn parse_number(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<u32>().ok()
}

#[cfg(test)]
#[path = "abi_test.rs"]
mod abi_test;
