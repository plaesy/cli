//! Router dan guard shell (ADR 0011). Murni: memetakan path ke halaman dan memeriksa izin.

use std::collections::BTreeSet;

/// Halaman yang dikenali shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    FlowList,
    FlowEditor(String),
    DataList,
    DataEditor(String),
    Document { kind: String, id: String },
    NotFound,
}

impl Page {
    /// Izin yang dibutuhkan halaman ini. `None` berarti tidak perlu izin.
    #[must_use]
    pub fn permission(&self) -> Option<&'static str> {
        match self {
            Self::FlowList | Self::FlowEditor(_) => Some("flow:read"),
            Self::DataList | Self::DataEditor(_) => Some("data:read"),
            Self::Document { .. } => Some("documents:read"),
            Self::NotFound => None,
        }
    }
}

/// Memetakan path ke halaman. Setiap segmen didekode dari percent-encoding, sehingga `%2F`
/// (atau `%2f`) di id atau kind menjadi `/`.
#[must_use]
pub fn route(path: &str) -> Page {
    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    match segments.as_slice() {
        [] | ["flows"] => Page::FlowList,
        ["flows", id] => Page::FlowEditor(decode_part(id)),
        ["data"] => Page::DataList,
        ["data", id] => Page::DataEditor(decode_part(id)),
        ["documents", kind, id] => Page::Document {
            kind: decode_part(kind),
            id: decode_part(id),
        },
        _ => Page::NotFound,
    }
}

/// Mengenkode satu segmen path. Huruf, angka, dan `-._~` tetap; byte lain di-percent-encode.
#[must_use]
pub fn encode_part(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Membalik [`encode_part`]. `%` yang tidak diikuti dua digit hex dibiarkan apa adanya.
#[must_use]
pub fn decode_part(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Some(value) = hex_pair(bytes[index + 1], bytes[index + 2]) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_pair(high: u8, low: u8) -> Option<u8> {
    let digit = |byte: u8| char::from(byte).to_digit(16).map(|value| value as u8);
    Some(digit(high)? * 16 + digit(low)?)
}

/// Guard izin: halaman dengan izin hanya boleh dibuka bila izin itu ada di daftar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guard {
    granted: BTreeSet<String>,
}

impl Guard {
    /// Guard dengan daftar izin.
    #[must_use]
    pub fn new(granted: &[&str]) -> Self {
        Self {
            granted: granted.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    /// Apakah halaman boleh dibuka.
    #[must_use]
    pub fn allows(&self, page: &Page) -> bool {
        match page.permission() {
            None => true,
            Some(permission) => self.granted.contains(permission),
        }
    }
}

/// Halaman daftar yang boleh tampil di navigasi untuk `guard`.
#[must_use]
pub fn nav(guard: &Guard) -> Vec<Page> {
    [Page::FlowList, Page::DataList]
        .into_iter()
        .filter(|page| guard.allows(page))
        .collect()
}

/// Path URL untuk halaman; kebalikan dari [`route`]. Setiap id dan kind di-percent-encode.
#[must_use]
pub fn path_for(page: &Page) -> String {
    match page {
        Page::FlowList => "/flows".to_owned(),
        Page::FlowEditor(id) => format!("/flows/{}", encode_part(id)),
        Page::DataList => "/data".to_owned(),
        Page::DataEditor(id) => format!("/data/{}", encode_part(id)),
        Page::Document { kind, id } => {
            format!("/documents/{}/{}", encode_part(kind), encode_part(id))
        }
        Page::NotFound => "/".to_owned(),
    }
}

/// Jejak navigasi untuk halaman: nama bagian, lalu id halaman bila ada.
#[must_use]
pub fn breadcrumb(page: &Page) -> Vec<String> {
    match page {
        Page::FlowList => vec!["Flow".to_owned()],
        Page::FlowEditor(id) => vec!["Flow".to_owned(), id.clone()],
        Page::DataList => vec!["Data".to_owned()],
        Page::DataEditor(id) => vec!["Data".to_owned(), id.clone()],
        Page::Document { kind, id } => vec!["Dokumen".to_owned(), kind.clone(), id.clone()],
        Page::NotFound => vec!["Tidak ditemukan".to_owned()],
    }
}

#[cfg(test)]
#[path = "shell_test.rs"]
mod shell_test;
