//! `impl Database` methods: queue, split out of `mod.rs`.

use super::*;

impl Database {
    /// Record a published file whose index update must be retried after a crash.
    pub async fn enqueue_index(&self, source_path: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO pending_index(source_path, queued_at) VALUES (?1, ?2)
             ON CONFLICT(source_path) DO UPDATE SET queued_at = excluded.queued_at",
            params![
                source_path.replace('\\', "/"),
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// Return durable index work items, oldest first.
    pub async fn pending_indexes(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT source_path FROM pending_index ORDER BY queued_at, source_path")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    /// Remove a work item only after its document was indexed successfully.
    pub async fn dequeue_index(&self, source_path: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM pending_index WHERE source_path = ?1",
            params![source_path.replace('\\', "/")],
        )
        .map_err(db_err)?;
        Ok(())
    }
}
