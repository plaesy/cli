//! Sinkronisasi dokumen (ADR 0003: adapter/sync). Keputusan default, didelegasikan dan bisa
//! dicabut: setiap perubahan diberi nomor versi monoton oleh satu sumber. Replika menarik
//! perubahan setelah versi terakhirnya dan menerapkannya dengan last-writer-wins per
//! `(kind, id)` berdasarkan versi. Perubahan yang lebih tua dari yang sudah diterapkan dibuang.
//!
//! Log perubahan lewat trait [`ChangeLog`]. Ada dua implementasi: [`MemoryChangeLog`] untuk
//! tes dan dry run, dan [`SqliteChangeLog`] yang bertahan setelah proses berhenti.

use plaesy_service::port::{check_key, DocumentStore, StoreError};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
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

/// Penyimpanan log perubahan. Versi harus naik monoton dan tidak boleh ada dua perubahan
/// dengan versi yang sama.
pub trait ChangeLog {
    /// Menambahkan perubahan. Harus sudah tersimpan saat fungsi kembali.
    ///
    /// # Errors
    ///
    /// Penyimpanan gagal.
    fn append(&self, change: &Change) -> Result<(), StoreError>;

    /// Versi tertinggi dalam log, atau 0 bila kosong.
    ///
    /// # Errors
    ///
    /// Penyimpanan gagal.
    fn head(&self) -> Result<u64, StoreError>;

    /// Perubahan dengan versi lebih besar dari `since`, dalam urutan versi.
    ///
    /// # Errors
    ///
    /// Penyimpanan gagal.
    fn since(&self, since: u64) -> Result<Vec<Change>, StoreError>;
}

/// Log di memori. Hilang saat proses berhenti.
#[derive(Debug, Default)]
pub struct MemoryChangeLog {
    changes: Mutex<Vec<Change>>,
}

impl ChangeLog for MemoryChangeLog {
    fn append(&self, change: &Change) -> Result<(), StoreError> {
        self.changes
            .lock()
            .map_err(|_| poisoned())?
            .push(change.clone());
        Ok(())
    }

    fn head(&self) -> Result<u64, StoreError> {
        let changes = self.changes.lock().map_err(|_| poisoned())?;
        Ok(changes.last().map_or(0, |change| change.version))
    }

    fn since(&self, since: u64) -> Result<Vec<Change>, StoreError> {
        let changes = self.changes.lock().map_err(|_| poisoned())?;
        Ok(changes
            .iter()
            .filter(|change| change.version > since)
            .cloned()
            .collect())
    }
}

/// Log di SQLite. Setiap perubahan disinkronkan ke disk sebelum `append` kembali.
#[derive(Debug)]
pub struct SqliteChangeLog {
    conn: Mutex<Connection>,
}

impl SqliteChangeLog {
    /// Membuka atau membuat log di `path`. Tabel dibuat bila belum ada.
    ///
    /// # Errors
    ///
    /// Galat penyimpanan.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path.as_ref()).map_err(|e| backend(&e))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sync_changes (
                 version INTEGER PRIMARY KEY,
                 kind TEXT NOT NULL,
                 id TEXT NOT NULL,
                 op TEXT NOT NULL CHECK (op IN ('put', 'delete')),
                 body TEXT
             );",
        )
        .map_err(|e| backend(&e))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl ChangeLog for SqliteChangeLog {
    fn append(&self, change: &Change) -> Result<(), StoreError> {
        let (op, body) = match &change.op {
            Op::Put(document) => (
                "put",
                Some(serde_json::to_string(document).map_err(|e| {
                    StoreError::Backend(format!("perubahan tidak bisa ditulis: {e}"))
                })?),
            ),
            Op::Delete => ("delete", None),
        };
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        conn.execute(
            "INSERT INTO sync_changes (version, kind, id, op, body) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                i64::try_from(change.version)
                    .map_err(|_| StoreError::Backend("versi terlalu besar".to_owned()))?,
                change.kind,
                change.id,
                op,
                body
            ],
        )
        .map(|_| ())
        .map_err(|e| backend(&e))
    }

    fn head(&self) -> Result<u64, StoreError> {
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        let head: Option<i64> = conn
            .query_row("SELECT MAX(version) FROM sync_changes", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| backend(&e))?
            .flatten();
        Ok(head.map_or(0, |v| u64::try_from(v).unwrap_or(0)))
    }

    fn since(&self, since: u64) -> Result<Vec<Change>, StoreError> {
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        let since = i64::try_from(since).unwrap_or(i64::MAX);
        let mut statement = conn
            .prepare(
                "SELECT version, kind, id, op, body FROM sync_changes
                 WHERE version > ?1 ORDER BY version",
            )
            .map_err(|e| backend(&e))?;
        let rows = statement
            .query_map(params![since], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| backend(&e))?;
        let mut changes = Vec::new();
        for row in rows {
            let (version, kind, id, op, body) = row.map_err(|e| backend(&e))?;
            let op = match (op.as_str(), body) {
                ("put", Some(text)) => Op::Put(
                    serde_json::from_str(&text)
                        .map_err(|e| StoreError::Backend(format!("perubahan rusak: {e}")))?,
                ),
                ("delete", None) => Op::Delete,
                _ => {
                    return Err(StoreError::Backend(
                        "baris log perubahan tidak valid".to_owned(),
                    ))
                }
            };
            changes.push(Change {
                version: u64::try_from(version).unwrap_or(0),
                kind,
                id,
                op,
            });
        }
        Ok(changes)
    }
}

/// Sumber perubahan: menerapkan operasi ke `store` dan mencatatnya di `log`.
#[derive(Debug)]
pub struct Source<S: DocumentStore, L: ChangeLog = MemoryChangeLog> {
    store: S,
    log: L,
}

impl<S: DocumentStore> Source<S, MemoryChangeLog> {
    /// Membungkus `store` dengan log di memori.
    #[must_use]
    pub fn new(store: S) -> Self {
        Self {
            store,
            log: MemoryChangeLog::default(),
        }
    }
}

impl<S: DocumentStore, L: ChangeLog> Source<S, L> {
    /// Membungkus `store` dengan log yang diberikan. Versi berikutnya melanjutkan `log.head()`.
    #[must_use]
    pub fn with_log(store: S, log: L) -> Self {
        Self { store, log }
    }

    /// Menyimpan dokumen dan mencatat perubahannya. Mengembalikan versi perubahan.
    ///
    /// # Errors
    ///
    /// Kunci tidak valid, atau penyimpanan gagal.
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

    /// Versi terakhir dalam log.
    ///
    /// # Errors
    ///
    /// Penyimpanan log gagal.
    pub fn head(&self) -> Result<u64, StoreError> {
        self.log.head()
    }

    /// Perubahan dengan versi lebih besar dari `since`, dalam urutan.
    ///
    /// # Errors
    ///
    /// Penyimpanan log gagal.
    pub fn changes_since(&self, since: u64) -> Result<Vec<Change>, StoreError> {
        self.log.since(since)
    }

    /// Penyimpanan yang dibungkus.
    pub fn store(&self) -> &S {
        &self.store
    }

    fn record(&self, kind: &str, id: &str, op: Op) -> Result<u64, StoreError> {
        let version = self.log.head()? + 1;
        self.log.append(&Change {
            version,
            kind: kind.to_owned(),
            id: id.to_owned(),
            op,
        })?;
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

fn backend(error: &rusqlite::Error) -> StoreError {
    StoreError::Backend(error.to_string())
}

fn poisoned() -> StoreError {
    StoreError::Backend("status sinkronisasi terkunci karena panik sebelumnya".to_owned())
}

#[cfg(test)]
#[path = "sync_test.rs"]
mod sync_test;
