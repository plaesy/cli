//! Capability plugin: apa yang diminta manifest, dan apa yang disetujui pengguna.
//!
//! Host hanya memberi irisan permintaan dan persetujuan. Hasil tidak pernah lebih lebar dari
//! permintaan. Kecocokan dibuat ketat: path dibandingkan per komponen, host harus sama persis,
//! dan kesalahan normalisasi menolak permintaan (fail closed).

use std::fmt;

use serde::Deserialize;

use crate::error::PluginError;

const MAX_PATH_LEN: usize = 4096;
const MAX_HOST_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

/// Satu capability. Bentuk JSON memakai field `kind`, misalnya `{"kind":"fs_read","path":"/data"}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Capability {
    /// Membaca berkas atau direktori di bawah `path`.
    FsRead { path: String },
    /// Menulis berkas atau direktori di bawah `path`. Tidak otomatis mencakup baca.
    FsWrite { path: String },
    /// Koneksi jaringan ke host tertentu. Tanpa wildcard dan tanpa port.
    NetHost { host: String },
    /// Akses jam sistem.
    Clock,
}

impl Capability {
    /// Mengembalikan capability dengan path dan host yang sudah dinormalisasi.
    ///
    /// # Errors
    ///
    /// Mengembalikan `PluginError::InvalidCapability` jika path atau host tidak valid.
    pub fn normalize(self) -> Result<Self, PluginError> {
        match self {
            Self::FsRead { path } => Ok(Self::FsRead {
                path: normalize_path(&path)?,
            }),
            Self::FsWrite { path } => Ok(Self::FsWrite {
                path: normalize_path(&path)?,
            }),
            Self::NetHost { host } => Ok(Self::NetHost {
                host: normalize_host(&host)?,
            }),
            Self::Clock => Ok(Self::Clock),
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FsRead { path } => write!(f, "fs_read:{path}"),
            Self::FsWrite { path } => write!(f, "fs_write:{path}"),
            Self::NetHost { host } => write!(f, "net_host:{host}"),
            Self::Clock => f.write_str("clock"),
        }
    }
}

/// Hasil perhitungan capability untuk satu plugin.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Grant {
    /// Capability yang diminta dan disetujui. Selalu sama atau lebih sempit dari permintaan.
    pub granted: Vec<Capability>,
    /// Capability yang diminta tetapi tidak disetujui. Host harus menahannya.
    pub denied: Vec<Capability>,
}

/// Menghitung irisan capability yang diminta dan disetujui.
///
/// Setiap permintaan diberikan jika ada persetujuan yang mencakupnya. Untuk path, persetujuan
/// mencakup permintaan jika sama atau merupakan induk per komponen. Hasil memakai path permintaan,
/// jadi tidak pernah lebih lebar dari yang diminta. Kedua input harus sudah dinormalisasi.
#[must_use]
pub fn grant(requested: &[Capability], approved: &[Capability]) -> Grant {
    let mut out = Grant::default();
    for request in requested {
        if approved.iter().any(|approval| covers(approval, request)) {
            out.granted.push(request.clone());
        } else {
            out.denied.push(request.clone());
        }
    }
    out
}

fn covers(approved: &Capability, requested: &Capability) -> bool {
    match (approved, requested) {
        (Capability::FsRead { path: a }, Capability::FsRead { path: r })
        | (Capability::FsWrite { path: a }, Capability::FsWrite { path: r }) => path_covers(a, r),
        (Capability::NetHost { host: a }, Capability::NetHost { host: r }) => a == r,
        (Capability::Clock, Capability::Clock) => true,
        _ => false,
    }
}

/// `approved` mencakup `requested` jika sama, atau `requested` berada di bawahnya per komponen.
fn path_covers(approved: &str, requested: &str) -> bool {
    if approved == requested {
        return true;
    }
    if approved.ends_with('/') {
        requested.starts_with(approved)
    } else {
        requested.starts_with(&format!("{approved}/"))
    }
}

/// Menormalkan path: `\` menjadi `/`, akhiran `/` dibuang (kecuali akar), dan komponen `.`, `..`,
/// atau kosong ditolak. Huruf drive `C:` hanya diizinkan di awal path relatif.
pub(crate) fn normalize_path(raw: &str) -> Result<String, PluginError> {
    let invalid = |why: &str| PluginError::InvalidCapability(format!("path {raw:?}: {why}"));
    if raw.is_empty() {
        return Err(invalid("kosong"));
    }
    if raw.len() > MAX_PATH_LEN {
        return Err(invalid("terlalu panjang"));
    }
    if raw.chars().any(char::is_control) {
        return Err(invalid("mengandung karakter kontrol"));
    }

    let unified = raw.replace('\\', "/");
    let trimmed = unified.trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok("/".to_owned());
    }
    if is_drive(trimmed) {
        return Ok(format!("{trimmed}/"));
    }

    let (absolute, rest) = match trimmed.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, trimmed),
    };
    let mut out = String::with_capacity(trimmed.len() + 1);
    if absolute {
        out.push('/');
    }
    for (index, part) in rest.split('/').enumerate() {
        if part.is_empty() || part == "." || part == ".." {
            return Err(invalid("komponen kosong, '.' atau '..' tidak diizinkan"));
        }
        let drive_allowed = index == 0 && !absolute && is_drive(part);
        if part.contains(':') && !drive_allowed {
            return Err(invalid("':' hanya boleh sebagai huruf drive di awal"));
        }
        if index > 0 {
            out.push('/');
        }
        out.push_str(part);
    }
    Ok(out)
}

/// Menormalkan host: huruf kecil ASCII, label dipisah '.', dan tanpa wildcard atau port.
pub(crate) fn normalize_host(raw: &str) -> Result<String, PluginError> {
    let host = raw.to_ascii_lowercase();
    let invalid = || {
        PluginError::InvalidCapability(format!(
            "host {raw:?} tidak valid: tanpa wildcard, port, atau karakter selain huruf, angka, dan '-'"
        ))
    };
    if host.is_empty() || host.len() > MAX_HOST_LEN {
        return Err(invalid());
    }
    for label in host.split('.') {
        let valid = !label.is_empty()
            && label.len() <= MAX_LABEL_LEN
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid {
            return Err(invalid());
        }
    }
    Ok(host)
}

/// Huruf drive Windows dalam bentuk `X:`.
fn is_drive(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
#[path = "capability_test.rs"]
mod capability_test;
