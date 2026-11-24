//! `impl Database` methods: transaction, split out of `mod.rs`.

use super::*;

impl Database {
    /// Begin a transaction.
    pub async fn begin_transaction(&self) -> Result<()> {
        self.conn
            .lock()
            .execute_batch("BEGIN")
            .map_err(|e| MemoryError::TransactionError {
                reason: e.to_string(),
            })
    }

    /// Commit current transaction.
    pub async fn commit(&self) -> Result<()> {
        self.conn
            .lock()
            .execute_batch("COMMIT")
            .map_err(|e| MemoryError::TransactionError {
                reason: e.to_string(),
            })
    }

    /// Rollback current transaction.
    pub async fn rollback(&self) -> Result<()> {
        self.conn
            .lock()
            .execute_batch("ROLLBACK")
            .map_err(|e| MemoryError::TransactionError {
                reason: e.to_string(),
            })
    }
}
