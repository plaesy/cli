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

/// Memetakan path ke halaman. `/documents/{kind}/{id}` menerima `%2F` atau `%2f` di `kind`.
#[must_use]
pub fn route(path: &str) -> Page {
    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    match segments.as_slice() {
        [] | ["flows"] => Page::FlowList,
        ["flows", id] => Page::FlowEditor((*id).to_owned()),
        ["data"] => Page::DataList,
        ["data", id] => Page::DataEditor((*id).to_owned()),
        ["documents", kind, id] => Page::Document {
            kind: kind.replace("%2F", "/").replace("%2f", "/"),
            id: (*id).to_owned(),
        },
        _ => Page::NotFound,
    }
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
