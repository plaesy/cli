//! Durable, explicitly acknowledged direct messages between agents.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

pub mod auth;
pub mod keys;
pub mod shared;
pub mod transport;

const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_ERROR_BYTES: usize = 1024;
/// Claims allowed per message. Once a message has used all of them and its last
/// attempt was released or its lease expired, the message becomes `gagal`.
pub const MAX_ATTEMPTS: u32 = 5;

/// Columns added after the first schema. Each one is added on open when missing.
const MIGRATED_COLUMNS: [(&str, &str); 6] = [
    ("lease_owner", "TEXT"),
    ("lease_expiry_epoch_ms", "INTEGER"),
    ("attempts", "INTEGER NOT NULL DEFAULT 0"),
    ("failed_at_epoch_ms", "INTEGER"),
    ("acked_at_epoch_ms", "INTEGER"),
    ("last_error", "TEXT"),
];

/// Delivery state of a message. The state is derived from stored columns and the
/// current time, so an expired lease is reported as `menunggu` without a rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// Waiting to be claimed (never claimed, or its last lease expired or was released).
    #[serde(rename = "menunggu")]
    Pending,
    /// Held by an active lease.
    #[serde(rename = "diambil")]
    Claimed,
    /// Acknowledged by the recipient.
    #[serde(rename = "selesai")]
    Done,
    /// Failed: released after the last attempt, or its last lease expired.
    #[serde(rename = "gagal")]
    Failed,
}

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
    pub status: MessageStatus,
    /// Number of claims made so far.
    pub attempts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
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

/// SQLite-backed durable queue; at-least-once retrieval ends only on explicit ack or failure.
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

    fn initialize(mut connection: Connection) -> Result<Self, MessageError> {
        connection.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS relay_messages (
               id TEXT PRIMARY KEY,
               sender TEXT NOT NULL,
               recipient TEXT NOT NULL,
               body TEXT NOT NULL,
               created_at TEXT NOT NULL,
               acknowledged_at TEXT,
               UNIQUE(sender, id)
             );
             CREATE INDEX IF NOT EXISTS relay_messages_inbox
               ON relay_messages(recipient, acknowledged_at, created_at);",
        )?;
        migrate_columns(&mut connection)?;
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
        let now = chrono::Utc::now();
        let created_at = now.to_rfc3339();
        let now_ms = now.timestamp_millis();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<Message> = tx
            .query_row(
                &format!("{} WHERE id=?1", select_messages(now_ms)),
                [id],
                row_to_message,
            )
            .optional()?;
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
            status: MessageStatus::Pending,
            attempts: 0,
            last_error: None,
        })
    }

    /// Returns unfinished messages for `recipient` in FIFO order without consuming them.
    /// Done and failed messages are excluded.
    pub fn inbox(&self, recipient: &str, limit: usize) -> Result<Vec<Message>, MessageError> {
        validate_agent(recipient)?;
        if limit == 0 || limit > 500 {
            return Err(MessageError::Invalid("inbox limit must be 1..=500"));
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let sql = format!(
            "{} WHERE recipient=?1 AND {} ORDER BY created_at,id LIMIT ?2",
            select_messages(now_ms),
            live_filter(now_ms)
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![recipient, limit as i64], row_to_message)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Returns one message by ID with its current status, or `None` when it does not exist.
    pub fn find(&self, id: &str) -> Result<Option<Message>, MessageError> {
        validate_id(id)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let message = self
            .connection
            .query_row(
                &format!("{} WHERE id=?1", select_messages(now_ms)),
                [id],
                row_to_message,
            )
            .optional()?;
        Ok(message)
    }

    /// Atomically claim the oldest available message for a consumer until a bounded expiry.
    /// Each claim counts as one attempt; messages at `MAX_ATTEMPTS` are not claimable.
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
                &format!(
                    "SELECT id FROM relay_messages WHERE recipient=?1 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND attempts<{MAX_ATTEMPTS} AND (lease_expiry_epoch_ms IS NULL OR lease_expiry_epoch_ms<=?2) ORDER BY created_at,id LIMIT 1"
                ),
                params![recipient, now_ms],
                |row| row.get(0),
            )
            .optional()?;
        let Some(id) = next else {
            tx.commit()?;
            return Ok(None);
        };
        let changed = tx.execute(
            &format!(
                "UPDATE relay_messages SET lease_owner=?1,lease_expiry_epoch_ms=?2,attempts=attempts+1 WHERE id=?3 AND recipient=?4 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND attempts<{MAX_ATTEMPTS} AND (lease_expiry_epoch_ms IS NULL OR lease_expiry_epoch_ms<=?5)"
            ),
            params![token, expiry_ms, id, recipient, now_ms],
        )?;
        if changed != 1 {
            return Err(MessageError::LeaseConflict);
        }
        let message = tx.query_row(
            &format!("{} WHERE id=?1", select_messages(now_ms)),
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
            "UPDATE relay_messages SET acknowledged_at=?1,acked_at_epoch_ms=?2,lease_owner=NULL,lease_expiry_epoch_ms=NULL WHERE id=?3 AND recipient=?4 AND lease_owner=?5 AND lease_expiry_epoch_ms>?6 AND acknowledged_at IS NULL",
            params![now_text, now_ms, id, recipient, owner, now_ms],
        )?;
        Ok(changed == 1)
    }

    /// Releases a claim after a failed attempt. The message returns to `menunggu` for
    /// another attempt, or becomes `gagal` when its last allowed attempt was used.
    /// Returns the resulting status, or `None` when the lease token is stale.
    pub fn release_claim(
        &mut self,
        id: &str,
        recipient: &str,
        owner: &str,
        error: &str,
    ) -> Result<Option<MessageStatus>, MessageError> {
        validate_id(id)?;
        validate_agent(recipient)?;
        validate_id(owner)?;
        if error.len() > MAX_ERROR_BYTES {
            return Err(MessageError::Invalid("error must be at most 1024 UTF-8 bytes"));
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let changed = tx.execute(
            &format!(
                "UPDATE relay_messages SET lease_owner=NULL,lease_expiry_epoch_ms=NULL,last_error=?1,failed_at_epoch_ms=CASE WHEN attempts>={MAX_ATTEMPTS} THEN ?2 ELSE NULL END WHERE id=?3 AND recipient=?4 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND lease_owner=?5 AND lease_expiry_epoch_ms>?2"
            ),
            params![error, now_ms, id, recipient, owner],
        )?;
        if changed != 1 {
            tx.commit()?;
            return Ok(None);
        }
        let failed: bool = tx.query_row(
            "SELECT failed_at_epoch_ms IS NOT NULL FROM relay_messages WHERE id=?1",
            [id],
            |row| row.get(0),
        )?;
        tx.commit()?;
        Ok(Some(if failed {
            MessageStatus::Failed
        } else {
            MessageStatus::Pending
        }))
    }

    /// Acknowledge only never-claimed messages for legacy callers; lease tokens cannot be bypassed.
    pub fn acknowledge(&mut self, id: &str, recipient: &str) -> Result<bool, MessageError> {
        validate_id(id)?;
        validate_agent(recipient)?;
        let now = chrono::Utc::now();
        let now_text = now.to_rfc3339();
        let changed = self.connection.execute(
            "UPDATE relay_messages SET acknowledged_at=?1,acked_at_epoch_ms=?2 WHERE id=?3 AND recipient=?4 AND acknowledged_at IS NULL AND attempts=0",
            params![now_text, now.timestamp_millis(), id, recipient],
        )?;
        Ok(changed == 1)
    }

    /// Deletes done messages whose acknowledgment is older than `older_than`.
    /// Failed and unfinished messages are kept. A purged ID is no longer deduplicated,
    /// so choose a retention period longer than any sender's retry window.
    pub fn purge_completed(&mut self, older_than: Duration) -> Result<usize, MessageError> {
        let age_ms = i64::try_from(older_than.as_millis()).unwrap_or(i64::MAX);
        let cutoff_ms = chrono::Utc::now()
            .timestamp_millis()
            .saturating_sub(age_ms);
        let removed = self.connection.execute(
            "DELETE FROM relay_messages WHERE acknowledged_at IS NOT NULL AND acked_at_epoch_ms IS NOT NULL AND acked_at_epoch_ms<=?1",
            params![cutoff_ms],
        )?;
        Ok(removed)
    }
}

/// Adds columns missing from an older database. Runs in one immediate transaction,
/// so concurrent openers cannot add the same column twice, and existing rows keep their data.
fn migrate_columns(connection: &mut Connection) -> Result<(), MessageError> {
    if missing_columns(connection)?.is_empty() {
        return Ok(());
    }
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let missing = missing_columns(&tx)?;
    if missing.is_empty() {
        tx.commit()?;
        return Ok(());
    }
    for (column, definition) in &missing {
        tx.execute_batch(&format!(
            "ALTER TABLE relay_messages ADD COLUMN {column} {definition};"
        ))?;
    }
    // Rows leased before `attempts` existed had exactly one claim.
    tx.execute_batch(
        "UPDATE relay_messages SET attempts=1 WHERE lease_owner IS NOT NULL AND acknowledged_at IS NULL AND attempts=0;",
    )?;
    backfill_acked_at_epoch(&tx)?;
    tx.commit()?;
    Ok(())
}

fn missing_columns(
    connection: &Connection,
) -> Result<Vec<(&'static str, &'static str)>, MessageError> {
    let existing: Vec<String> = connection
        .prepare("PRAGMA table_info(relay_messages)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<String>, rusqlite::Error>>()?;
    Ok(MIGRATED_COLUMNS
        .iter()
        .copied()
        .filter(|(column, _)| !existing.iter().any(|name| name.as_str() == *column))
        .collect())
}

/// Sets the numeric acknowledgment time for legacy rows from their RFC 3339 text.
/// Rows whose text cannot be parsed keep NULL and are never purged.
fn backfill_acked_at_epoch(connection: &Connection) -> Result<(), MessageError> {
    let legacy: Vec<(String, String)> = connection
        .prepare(
            "SELECT id,acknowledged_at FROM relay_messages WHERE acknowledged_at IS NOT NULL AND acked_at_epoch_ms IS NULL",
        )?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<(String, String)>, rusqlite::Error>>()?;
    for (id, acknowledged_at) in legacy {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(&acknowledged_at) {
            connection.execute(
                "UPDATE relay_messages SET acked_at_epoch_ms=?1 WHERE id=?2",
                params![parsed.timestamp_millis(), id],
            )?;
        }
    }
    Ok(())
}

/// SQL expression that derives the status of a row at `now_ms`.
fn status_sql(now_ms: i64) -> String {
    format!(
        "CASE WHEN acknowledged_at IS NOT NULL THEN 'selesai' \
         WHEN failed_at_epoch_ms IS NOT NULL THEN 'gagal' \
         WHEN lease_owner IS NOT NULL AND lease_expiry_epoch_ms>{now_ms} THEN 'diambil' \
         WHEN attempts>={MAX_ATTEMPTS} THEN 'gagal' \
         ELSE 'menunggu' END"
    )
}

/// SELECT clause that returns every `Message` column plus the derived status.
fn select_messages(now_ms: i64) -> String {
    format!(
        "SELECT id,sender,recipient,body,created_at,acknowledged_at,lease_owner,lease_expiry_epoch_ms,{status},attempts,last_error FROM relay_messages",
        status = status_sql(now_ms)
    )
}

/// Filter for messages that are neither done nor failed at `now_ms`.
fn live_filter(now_ms: i64) -> String {
    format!(
        "acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND (attempts<{MAX_ATTEMPTS} OR lease_expiry_epoch_ms>{now_ms})"
    )
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
fn parse_status(text: &str) -> rusqlite::Result<MessageStatus> {
    match text {
        "menunggu" => Ok(MessageStatus::Pending),
        "diambil" => Ok(MessageStatus::Claimed),
        "selesai" => Ok(MessageStatus::Done),
        "gagal" => Ok(MessageStatus::Failed),
        other => Err(rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            format!("unknown message status {other}").into(),
        )),
    }
}
/// Maps a row selected by `select_messages`; the column order must match that clause.
fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let status: String = row.get(8)?;
    Ok(Message {
        id: row.get(0)?,
        sender: row.get(1)?,
        recipient: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
        acknowledged_at: row.get(5)?,
        lease_owner: row.get(6)?,
        lease_expiry_epoch_ms: row.get(7)?,
        status: parse_status(&status)?,
        attempts: row.get(9)?,
        last_error: row.get(10)?,
    })
}

#[cfg(test)]
#[path = "relay_test.rs"]
mod relay_test;
