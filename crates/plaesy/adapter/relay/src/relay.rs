//! Durable, explicitly acknowledged direct messages between agents.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

pub mod auth;
pub mod keys;
pub mod shared;
pub mod transport;

const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub recipient: String,
    pub body: String,
    pub created_at: String,
    pub acknowledged_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_expiry_epoch_ms: Option<i64>,
}

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("invalid message: {0}")]
    Invalid(&'static str),
    #[error("message idempotency key was reused with different content")]
    IdempotencyConflict,
    #[error("lease duration must be 1..=86400 seconds")]
    InvalidLease,
    #[error("message is not claimable or lease token is stale")]
    LeaseConflict,
    #[error(transparent)]
    Storage(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database or its parent directory permissions expose it to group/other users")]
    InsecurePermissions,
}

/// SQLite-backed durable queue; at-least-once retrieval ends only on explicit ack.
pub struct MessageStore {
    connection: Connection,
}

#[cfg(unix)]
fn create_private_file_if_missing(path: &Path) -> Result<(), MessageError> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Ok(std::fs::File::open(path)?)
            } else {
                Err(error)
            }
        })?;
    Ok(())
}

#[cfg(unix)]
fn verify_private_file(path: &Path) -> Result<(), MessageError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)?.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(MessageError::InsecurePermissions);
    }
    Ok(())
}

impl MessageStore {
    /// Opens or creates a durable SQLite queue at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MessageError> {
        let path = path.as_ref();
        #[cfg(unix)]
        create_private_file_if_missing(path)?;
        #[cfg(unix)]
        verify_private_file(path)?;
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Self::initialize(connection)
    }

    /// Creates an in-memory queue (useful for tests and single-process callers).
    pub fn in_memory() -> Result<Self, MessageError> {
        Self::initialize(Connection::open_in_memory()?)
    }

    fn initialize(connection: Connection) -> Result<Self, MessageError> {
        connection.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS relay_messages (
               id TEXT PRIMARY KEY,
               sender TEXT NOT NULL,
               recipient TEXT NOT NULL,
               body TEXT NOT NULL,
               created_at TEXT NOT NULL,
               acknowledged_at TEXT,
               lease_owner TEXT,
               lease_expiry_epoch_ms INTEGER,
               UNIQUE(sender, id)
             );
             CREATE INDEX IF NOT EXISTS relay_messages_inbox
               ON relay_messages(recipient, acknowledged_at, created_at);",
        )?;
        for (column, definition) in [
            ("lease_owner", "TEXT"),
            ("lease_expiry_epoch_ms", "INTEGER"),
        ] {
            let exists = connection
                .prepare("PRAGMA table_info(relay_messages)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .any(|existing| existing == column);
            if !exists {
                connection.execute_batch(&format!(
                    "ALTER TABLE relay_messages ADD COLUMN {column} {definition};"
                ))?;
            }
        }
        Ok(Self { connection })
    }

    /// Enqueues a message; reusing `id` with identical content is idempotent.
    pub fn send(
        &mut self,
        id: &str,
        sender: &str,
        recipient: &str,
        body: &str,
    ) -> Result<Message, MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        validate_agent(recipient)?;
        if body.is_empty() || body.len() > MAX_BODY_BYTES {
            return Err(MessageError::Invalid("body must be 1..=65536 UTF-8 bytes"));
        }
        let created_at = chrono::Utc::now().to_rfc3339();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<Message> = tx.query_row(
            "SELECT id,sender,recipient,body,created_at,acknowledged_at,lease_owner,lease_expiry_epoch_ms FROM relay_messages WHERE id=?1",
            [id], row_to_message,
        ).optional()?;
        if let Some(message) = existing {
            if message.sender != sender || message.recipient != recipient || message.body != body {
                return Err(MessageError::IdempotencyConflict);
            }
            tx.commit()?;
            return Ok(message);
        }
        tx.execute(
            "INSERT INTO relay_messages(id,sender,recipient,body,created_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, sender, recipient, body, created_at],
        )?;
        tx.commit()?;
        Ok(Message {
            id: id.into(),
            sender: sender.into(),
            recipient: recipient.into(),
            body: body.into(),
            created_at,
            acknowledged_at: None,
            lease_owner: None,
            lease_expiry_epoch_ms: None,
        })
    }

    /// Returns pending messages for `recipient` in FIFO order without consuming them.
    pub fn inbox(&self, recipient: &str, limit: usize) -> Result<Vec<Message>, MessageError> {
        validate_agent(recipient)?;
        if limit == 0 || limit > 500 {
            return Err(MessageError::Invalid("inbox limit must be 1..=500"));
        }
        let mut statement = self.connection.prepare(
            "SELECT id,sender,recipient,body,created_at,acknowledged_at,lease_owner,lease_expiry_epoch_ms FROM relay_messages WHERE recipient=?1 AND acknowledged_at IS NULL ORDER BY created_at,id LIMIT ?2",
        )?;
        let rows = statement.query_map(params![recipient, limit as i64], row_to_message)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Atomically claim the oldest available message for a consumer until a bounded expiry.
    pub fn claim_next(
        &mut self,
        recipient: &str,
        lease_seconds: u64,
    ) -> Result<Option<Message>, MessageError> {
        validate_agent(recipient)?;
        if lease_seconds == 0 || lease_seconds > 86_400 {
            return Err(MessageError::InvalidLease);
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let expiry_ms = now_ms + (lease_seconds as i64 * 1_000);
        let token = uuid::Uuid::new_v4().to_string();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let next: Option<String> = tx
            .query_row(
                "SELECT id FROM relay_messages WHERE recipient=?1 AND acknowledged_at IS NULL AND (lease_expiry_epoch_ms IS NULL OR lease_expiry_epoch_ms<=?2) ORDER BY created_at,id LIMIT 1",
                params![recipient, now_ms],
                |row| row.get(0),
            )
            .optional()?;
        let Some(id) = next else {
            tx.commit()?;
            return Ok(None);
        };
        let changed = tx.execute(
            "UPDATE relay_messages SET lease_owner=?1,lease_expiry_epoch_ms=?2 WHERE id=?3 AND recipient=?4 AND acknowledged_at IS NULL AND (lease_expiry_epoch_ms IS NULL OR lease_expiry_epoch_ms<=?5)",
            params![token, expiry_ms, id, recipient, now_ms],
        )?;
        if changed != 1 {
            return Err(MessageError::LeaseConflict);
        }
        let message = tx.query_row(
            "SELECT id,sender,recipient,body,created_at,acknowledged_at,lease_owner,lease_expiry_epoch_ms FROM relay_messages WHERE id=?1",
            [&id],
            row_to_message,
        )?;
        tx.commit()?;
        Ok(Some(message))
    }

    /// Acknowledge a claimed message only when the caller still owns its lease.
    pub fn acknowledge_claim(
        &mut self,
        id: &str,
        recipient: &str,
        owner: &str,
    ) -> Result<bool, MessageError> {
        validate_id(id)?;
        validate_agent(recipient)?;
        validate_id(owner)?;
        let now = chrono::Utc::now();
        let now_text = now.to_rfc3339();
        let now_ms = now.timestamp_millis();
        let changed = self.connection.execute(
            "UPDATE relay_messages SET acknowledged_at=?1,lease_owner=NULL,lease_expiry_epoch_ms=NULL WHERE id=?2 AND recipient=?3 AND lease_owner=?4 AND lease_expiry_epoch_ms>?5 AND acknowledged_at IS NULL",
            params![now_text, id, recipient, owner, now_ms],
        )?;
        Ok(changed == 1)
    }

    /// Acknowledge only never-claimed messages for legacy callers; lease tokens cannot be bypassed.
    pub fn acknowledge(&mut self, id: &str, recipient: &str) -> Result<bool, MessageError> {
        validate_id(id)?;
        validate_agent(recipient)?;
        let now = chrono::Utc::now();
        let now_text = now.to_rfc3339();
        let changed = self.connection.execute(
            "UPDATE relay_messages SET acknowledged_at=?1 WHERE id=?2 AND recipient=?3 AND acknowledged_at IS NULL AND lease_owner IS NULL",
            params![now_text, id, recipient],
        )?;
        Ok(changed == 1)
    }
}

fn validate_id(id: &str) -> Result<(), MessageError> {
    if id.len() > 128 || id.is_empty() || id.chars().any(char::is_control) {
        return Err(MessageError::Invalid(
            "id must be 1..=128 printable characters",
        ));
    }
    Ok(())
}
fn validate_agent(agent: &str) -> Result<(), MessageError> {
    if agent.is_empty()
        || agent.len() > 128
        || agent
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
    {
        return Err(MessageError::Invalid(
            "agent id must be 1..=128 ASCII letters, digits, '.', '_' or '-'",
        ));
    }
    Ok(())
}
fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        sender: row.get(1)?,
        recipient: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
        acknowledged_at: row.get(5)?,
        lease_owner: row.get(6)?,
        lease_expiry_epoch_ms: row.get(7)?,
    })
}

#[cfg(test)]
#[path = "relay_test.rs"]
mod relay_test;
