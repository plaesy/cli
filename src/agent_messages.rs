//! Durable, explicitly acknowledged direct messages between agents.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub recipient: String,
    pub body: String,
    pub created_at: String,
    pub acknowledged_at: Option<String>,
}

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("invalid message: {0}")]
    Invalid(&'static str),
    #[error("message idempotency key was reused with different content")]
    IdempotencyConflict,
    #[error(transparent)]
    Storage(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("database permissions expose it to group/other users")]
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
             CREATE TABLE IF NOT EXISTS agent_messages (
               id TEXT PRIMARY KEY,
               sender TEXT NOT NULL,
               recipient TEXT NOT NULL,
               body TEXT NOT NULL,
               created_at TEXT NOT NULL,
               acknowledged_at TEXT,
               UNIQUE(sender, id)
             );
             CREATE INDEX IF NOT EXISTS agent_messages_inbox
               ON agent_messages(recipient, acknowledged_at, created_at);",
        )?;
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
            "SELECT id,sender,recipient,body,created_at,acknowledged_at FROM agent_messages WHERE id=?1",
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
            "INSERT INTO agent_messages(id,sender,recipient,body,created_at) VALUES (?1,?2,?3,?4,?5)",
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
        })
    }

    /// Returns pending messages for `recipient` in FIFO order without consuming them.
    pub fn inbox(&self, recipient: &str, limit: usize) -> Result<Vec<Message>, MessageError> {
        validate_agent(recipient)?;
        if limit == 0 || limit > 500 {
            return Err(MessageError::Invalid("inbox limit must be 1..=500"));
        }
        let mut statement = self.connection.prepare(
            "SELECT id,sender,recipient,body,created_at,acknowledged_at FROM agent_messages WHERE recipient=?1 AND acknowledged_at IS NULL ORDER BY created_at,id LIMIT ?2",
        )?;
        let rows = statement.query_map(params![recipient, limit as i64], row_to_message)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Acknowledges only a pending message addressed to the specified recipient.
    pub fn acknowledge(&mut self, id: &str, recipient: &str) -> Result<bool, MessageError> {
        validate_id(id)?;
        validate_agent(recipient)?;
        let changed = self.connection.execute(
            "UPDATE agent_messages SET acknowledged_at=?1 WHERE id=?2 AND recipient=?3 AND acknowledged_at IS NULL",
            params![chrono::Utc::now().to_rfc3339(), id, recipient],
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn idempotent_send_and_explicit_ack_preserve_queue_semantics() {
        let mut store = MessageStore::in_memory().unwrap();
        let first = store.send("m-1", "agent-a", "agent-b", "hello").unwrap();
        assert_eq!(
            store.send("m-1", "agent-a", "agent-b", "hello").unwrap(),
            first
        );
        assert!(matches!(
            store.send("m-1", "agent-a", "agent-b", "changed"),
            Err(MessageError::IdempotencyConflict)
        ));
        assert_eq!(store.inbox("agent-b", 10).unwrap().len(), 1);
        assert!(!store.acknowledge("m-1", "agent-a").unwrap());
        assert!(store.acknowledge("m-1", "agent-b").unwrap());
        assert!(!store.acknowledge("m-1", "agent-b").unwrap());
        assert!(store.inbox("agent-b", 10).unwrap().is_empty());
    }

    #[test]
    fn concurrent_idempotent_retries_create_only_one_message() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("messages.sqlite");
        MessageStore::open(&path).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut store = MessageStore::open(path).unwrap();
                    barrier.wait();
                    store.send("same-id", "a", "b", "payload").unwrap()
                })
            })
            .collect();
        barrier.wait();
        let messages: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
        assert_eq!(messages[0], messages[1]);
        let store = MessageStore::open(&path).unwrap();
        assert_eq!(store.inbox("b", 10).unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn existing_database_exposed_to_other_users_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("shared.sqlite");
        std::fs::File::create(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            MessageStore::open(&path),
            Err(MessageError::InsecurePermissions)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn newly_created_database_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("private.sqlite");
        MessageStore::open(&path).unwrap();
        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o077, 0);
    }

    #[cfg(unix)]
    #[test]
    fn restrictive_permissions_on_existing_database_are_preserved() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("private.sqlite");
        std::fs::File::create(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        MessageStore::open(&path).unwrap();
        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn pending_message_survives_store_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("messages.sqlite");
        MessageStore::open(&path)
            .unwrap()
            .send("m-2", "a", "b", "offline")
            .unwrap();
        let reopened = MessageStore::open(&path).unwrap();
        assert_eq!(reopened.inbox("b", 10).unwrap()[0].body, "offline");
    }

    #[test]
    fn idempotent_resend_after_ack_keeps_message_acknowledged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("messages.sqlite");
        {
            let mut store = MessageStore::open(&path).unwrap();
            store.send("m-3", "a", "b", "done").unwrap();
            assert!(store.acknowledge("m-3", "b").unwrap());
        }
        let mut reopened = MessageStore::open(&path).unwrap();
        let resent = reopened.send("m-3", "a", "b", "done").unwrap();
        assert!(resent.acknowledged_at.is_some());
        assert!(reopened.inbox("b", 10).unwrap().is_empty());
    }

    #[test]
    fn invalid_recipient_and_oversized_body_are_rejected() {
        let mut store = MessageStore::in_memory().unwrap();
        assert!(matches!(
            store.send("m", "a", "../b", "hello"),
            Err(MessageError::Invalid(_))
        ));
        assert!(matches!(
            store.send("m", "a", "b", &"x".repeat(MAX_BODY_BYTES + 1)),
            Err(MessageError::Invalid(_))
        ));
    }
}
