//! Sinkronisasi dokumen (ADR 0003: adapter/sync). Keputusan default, didelegasikan dan bisa
//! dicabut: setiap perubahan diberi nomor versi monoton oleh satu sumber. Replika menarik
//! perubahan setelah versi terakhirnya dan menerapkannya dengan last-writer-wins per
//! `(kind, id)` berdasarkan versi. Perubahan yang lebih tua dari yang sudah diterapkan dibuang.
//!
//! Batasan: log perubahan ada di memori. Persistensinya dan pengiriman lewat socket belum ada.

use plaesy_service::port::{check_key, DocumentStore, StoreError};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// Operasi pada satu dokumen.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Dokumen disimpan atau ditimpa dengan isi ini.
    Put(Value),
    /// Dokumen dihapus.
    Delete,
}

/// Satu perubahan dalam log.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub version: u64,
    pub kind: String,
    pub id: String,
    pub op: Op,
}

/// Sumber perubahan: menerapkan operasi ke `store` dan mencatatnya dalam log.
#[derive(Debug)]
pub struct Source<S: DocumentStore> {
    store: S,
    log: Mutex<Vec<Change>>,
}

impl<S: DocumentStore> Source<S> {
    /// Membungkus `store`. Log dimulai kosong.
    #[must_use]
    pub fn new(store: S) -> Self {
        Self {
            store,
            log: Mutex::new(Vec::new()),
        }
    }

    /// Menyimpan dokumen dan mencatat perubahannya. Mengembalikan versi perubahan.
    ///
    /// # Errors
    ///
    /// Kunci tidak valid, atau penyimpanan gagal. Log tidak berubah bila penyimpanan gagal.
    pub fn put(&self, kind: &str, id: &str, document: &Value) -> Result<u64, StoreError> {
        self.store.put(kind, id, document)?;
        self.record(kind, id, Op::Put(document.clone()))
    }

    /// Menghapus dokumen dan mencatat perubahannya. Mengembalikan versi perubahan.
    ///
    /// # Errors
    ///
    /// Kunci tidak valid, atau penyimpanan gagal.
    pub fn delete(&self, kind: &str, id: &str) -> Result<u64, StoreError> {
        self.store.delete(kind, id)?;
        self.record(kind, id, Op::Delete)
    }

    /// Versi terakhir dalam log, atau 0 bila kosong.
    ///
    /// # Errors
    ///
    /// Kunci log terkunci karena panik sebelumnya.
    pub fn head(&self) -> Result<u64, StoreError> {
        let log = self.log.lock().map_err(|_| poisoned())?;
        Ok(log.last().map_or(0, |change| change.version))
    }

    /// Perubahan dengan versi lebih besar dari `since`, dalam urutan.
    ///
    /// # Errors
    ///
    /// Kunci log terkunci karena panik sebelumnya.
    pub fn changes_since(&self, since: u64) -> Result<Vec<Change>, StoreError> {
        let log = self.log.lock().map_err(|_| poisoned())?;
        Ok(log
            .iter()
            .filter(|change| change.version > since)
            .cloned()
            .collect())
    }

    /// Penyimpanan yang dibungkus.
    pub fn store(&self) -> &S {
        &self.store
    }

    fn record(&self, kind: &str, id: &str, op: Op) -> Result<u64, StoreError> {
        let mut log = self.log.lock().map_err(|_| poisoned())?;
        let version = log.last().map_or(1, |change| change.version + 1);
        log.push(Change {
            version,
            kind: kind.to_owned(),
            id: id.to_owned(),
            op,
        });
        Ok(version)
    }
}

/// Replika: menerapkan perubahan dari sumber ke `store` lokalnya.
#[derive(Debug)]
pub struct Replica<S: DocumentStore> {
    store: S,
    applied: Mutex<HashMap<(String, String), u64>>,
}

impl<S: DocumentStore> Replica<S> {
    /// Membungkus penyimpanan lokal. Belum ada perubahan yang diterapkan.
    #[must_use]
    pub fn new(store: S) -> Self {
        Self {
            store,
            applied: Mutex::new(HashMap::new()),
        }
    }

    /// Menerapkan satu perubahan. Mengembalikan `false` bila perubahan lebih tua dari yang
    /// sudah diterapkan untuk dokumen itu, sehingga dibuang.
    ///
    /// # Errors
    ///
    /// Kunci tidak valid, atau penyimpanan lokal gagal.
    pub fn apply(&self, change: &Change) -> Result<bool, StoreError> {
        check_key("kind", &change.kind)?;
        check_key("id", &change.id)?;
        let key = (change.kind.clone(), change.id.clone());
        let mut applied = self.applied.lock().map_err(|_| poisoned())?;
        if applied
            .get(&key)
            .is_some_and(|&version| version >= change.version)
        {
            return Ok(false);
        }
        match &change.op {
            Op::Put(document) => self.store.put(&change.kind, &change.id, document)?,
            Op::Delete => {
                self.store.delete(&change.kind, &change.id)?;
            }
        }
        applied.insert(key, change.version);
        Ok(true)
    }

    /// Menerapkan daftar perubahan berurutan. Mengembalikan jumlah yang benar-benar diterapkan.
    ///
    /// # Errors
    ///
    /// Galat pertama dari [`Replica::apply`].
    pub fn apply_all(&self, changes: &[Change]) -> Result<usize, StoreError> {
        let mut count = 0;
        for change in changes {
            if self.apply(change)? {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Penyimpanan lokal.
    pub fn store(&self) -> &S {
        &self.store
    }
}

fn poisoned() -> StoreError {
    StoreError::Backend("status sinkronisasi terkunci karena panik sebelumnya".to_owned())
}

#[cfg(test)]
#[path = "sync_test.rs"]
mod sync_test;
