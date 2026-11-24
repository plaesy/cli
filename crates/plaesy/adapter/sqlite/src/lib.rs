//! Penyimpanan dokumen di SQLite (ADR 0003: adapter `sqlite`). Mengimplementasikan port
//! `plaesy_service::port::DocumentStore`.
//!
//! Skema dimigrasikan saat database dibuka. Versi skema disimpan di `PRAGMA user_version`.
//! Database dengan versi lebih baru ditolak, tanpa mengubahnya.

use plaesy_service::port::{check_key, DocumentStore, StoreError};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

/// Versi skema terbaru yang didukung adapter ini.
pub const SCHEMA_VERSION: i64 = 1;

/// Penyimpanan dokumen di satu berkas SQLite.
#[derive(Debug)]
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

fn backend(error: &rusqlite::Error) -> StoreError {
    StoreError::Backend(error.to_string())
}

fn poisoned() -> StoreError {
    StoreError::Backend("koneksi terkunci karena panik sebelumnya".to_owned())
}

impl SqliteStore {
    /// Membuka (atau membuat) database di `path` dan memigrasikan skemanya.
    ///
    /// # Errors
    ///
    /// Galat penyimpanan, atau [`StoreError::UnsupportedSchema`] bila versi skema lebih baru.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path.as_ref()).map_err(|e| backend(&e))?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Versi skema yang sedang tersimpan di database.
    ///
    /// # Errors
    ///
    /// Galat penyimpanan.
    pub fn schema_version(&self) -> Result<i64, StoreError> {
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        read_version(&conn)
    }
}

fn read_version(conn: &Connection) -> Result<i64, StoreError> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| backend(&e))
}

fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let found = read_version(conn)?;
    if found > SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    if found == SCHEMA_VERSION {
        return Ok(());
    }
    // Versi 0: database baru atau kosong. Semua langkah dalam satu transaksi.
    conn.execute_batch(
        "BEGIN;
         CREATE TABLE IF NOT EXISTS documents (
             kind TEXT NOT NULL,
             id TEXT NOT NULL,
             body TEXT NOT NULL,
             PRIMARY KEY (kind, id)
         );
         PRAGMA user_version = 1;
         COMMIT;",
    )
    .map_err(|e| backend(&e))
}

impl DocumentStore for SqliteStore {
    fn put(&self, kind: &str, id: &str, document: &Value) -> Result<(), StoreError> {
        check_key("kind", kind)?;
        check_key("id", id)?;
        let body = serde_json::to_string(document)
            .map_err(|e| StoreError::Backend(format!("dokumen tidak bisa ditulis: {e}")))?;
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        conn.execute(
            "INSERT INTO documents (kind, id, body) VALUES (?1, ?2, ?3)
             ON CONFLICT (kind, id) DO UPDATE SET body = excluded.body",
            params![kind, id, body],
        )
        .map(|_| ())
        .map_err(|e| backend(&e))
    }

    fn get(&self, kind: &str, id: &str) -> Result<Option<Value>, StoreError> {
        check_key("kind", kind)?;
        check_key("id", id)?;
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        let body: Option<String> = conn
            .query_row(
                "SELECT body FROM documents WHERE kind = ?1 AND id = ?2",
                params![kind, id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| backend(&e))?;
        body.map(|text| {
            serde_json::from_str(&text)
                .map_err(|e| StoreError::Backend(format!("dokumen rusak: {e}")))
        })
        .transpose()
    }

    fn list(&self, kind: &str) -> Result<Vec<String>, StoreError> {
        check_key("kind", kind)?;
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        let mut statement = conn
            .prepare("SELECT id FROM documents WHERE kind = ?1 ORDER BY id")
            .map_err(|e| backend(&e))?;
        let rows = statement
            .query_map(params![kind], |row| row.get::<_, String>(0))
            .map_err(|e| backend(&e))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| backend(&e))
    }

    fn delete(&self, kind: &str, id: &str) -> Result<bool, StoreError> {
        check_key("kind", kind)?;
        check_key("id", id)?;
        let conn = self.conn.lock().map_err(|_| poisoned())?;
        let changed = conn
            .execute(
                "DELETE FROM documents WHERE kind = ?1 AND id = ?2",
                params![kind, id],
            )
            .map_err(|e| backend(&e))?;
        Ok(changed > 0)
    }
}

#[cfg(test)]
#[path = "sqlite_test.rs"]
mod sqlite_test;
