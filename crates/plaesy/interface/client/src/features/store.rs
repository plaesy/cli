//! Penyimpanan dokumen di memori untuk sesi ini. Ia mengimplementasi [`ApiClient`] sehingga
//! services fitur bisa dipakai tanpa jaringan. Data hilang saat halaman dimuat ulang.

use super::{ApiClient, ApiRequest};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Dokumen disimpan per path, dengan isi JSON mentah sebagai teks.
#[derive(Debug, Default, PartialEq)]
pub struct MemoryStore {
    documents: RefCell<BTreeMap<String, String>>,
}

impl MemoryStore {
    /// Path dokumen yang diawali `prefix`, terurut.
    #[must_use]
    pub fn paths_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.documents
            .borrow()
            .keys()
            .filter(|path| path.starts_with(prefix))
            .cloned()
            .collect()
    }
}

impl ApiClient for MemoryStore {
    fn send(&self, request: ApiRequest) -> Result<String, String> {
        match request.method {
            "PUT" => {
                let body = request.body.ok_or_else(|| "PUT tanpa body".to_owned())?;
                self.documents.borrow_mut().insert(request.path, body);
                Ok("{\"ok\":true}".to_owned())
            }
            "GET" => self
                .documents
                .borrow()
                .get(&request.path)
                .cloned()
                .ok_or_else(|| "tidak ditemukan".to_owned()),
            other => Err(format!("metode {other} tidak didukung")),
        }
    }
}

#[cfg(test)]
#[path = "store_test.rs"]
mod store_test;
