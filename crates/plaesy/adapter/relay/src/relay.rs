//! Durable, explicitly acknowledged direct messages between agents.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

use crate::task::{self, TaskState};
use thiserror::Error;

pub mod auth;
pub mod keys;
pub mod shared;
pub mod transport;
pub mod users;

const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_ERROR_BYTES: usize = 1024;
/// Claims allowed per message. Once a message has used all of them and its last
/// attempt was released or its lease expired, the message becomes `gagal`.
pub const MAX_ATTEMPTS: u32 = 5;

/// Columns added after the first schema. Each one is added on open when missing.
const MIGRATED_COLUMNS: [(&str, &str); 9] = [
    ("lease_owner", "TEXT"),
    ("lease_expiry_epoch_ms", "INTEGER"),
    ("attempts", "INTEGER NOT NULL DEFAULT 0"),
    ("failed_at_epoch_ms", "INTEGER"),
    ("acked_at_epoch_ms", "INTEGER"),
    ("last_error", "TEXT"),
    ("awaiting_input", "INTEGER NOT NULL DEFAULT 0"),
    ("input_prompt", "TEXT"),
    ("input_reply", "TEXT"),
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
    /// The recipient asked for input and is waiting for an answer. Not claimable until answered.
    #[serde(rename = "menunggu-input")]
    AwaitingInput,
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
    /// Question the recipient asked while the task is `input-required`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_prompt: Option<String>,
    /// Answer given by the sender; set when the task resumes, and delivered with the next claim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_reply: Option<String>,
}

/// A task's state, plus the question when it is `input-required`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskView {
    pub state: TaskState,
    pub prompt: Option<String>,
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
    #[error("no task with id {0}")]
    TaskNotFound(String),
    #[error("{0}")]
    TaskTransition(String),
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
               id TEXT NOT NULL,
               sender TEXT NOT NULL,
               recipient TEXT NOT NULL,
               body TEXT NOT NULL,
               created_at TEXT NOT NULL,
               acknowledged_at TEXT,
               PRIMARY KEY(sender, id)
             );
             CREATE INDEX IF NOT EXISTS relay_messages_inbox
               ON relay_messages(recipient, acknowledged_at, created_at);
             CREATE TABLE IF NOT EXISTS relay_tasks (
               id TEXT NOT NULL,
               sender TEXT NOT NULL,
               recipient TEXT NOT NULL,
               state TEXT NOT NULL,
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               PRIMARY KEY(sender, id)
             );",
        )?;
        migrate_columns(&mut connection)?;
        migrate_primary_key(&mut connection)?;
        Ok(Self { connection })
    }

    /// Opens a task in `submitted` state from `sender` to `recipient`.
    /// Reusing an existing `(sender, id)` is an error, so a task id cannot be re-opened.
    pub fn create_task(
        &mut self,
        id: &str,
        sender: &str,
        recipient: &str,
    ) -> Result<TaskState, MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        validate_agent(recipient)?;
        let now = chrono::Utc::now().to_rfc3339();
        self.connection.execute(
            "INSERT INTO relay_tasks (id, sender, recipient, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'submitted', ?4, ?4)",
            rusqlite::params![id, sender, recipient, now],
        )?;
        Ok(TaskState::Submitted)
    }

    /// Current state of a task, or `None` when no task has that sender and id.
    pub fn task_state(&self, sender: &str, id: &str) -> Result<Option<TaskState>, MessageError> {
        Ok(task_state_in(&self.connection, sender, id)?)
    }

    /// Queues a message and opens its task in one transaction, so a message never exists without
    /// its task. Repeating the same send changes nothing and returns the task's current state.
    /// Reusing the id with different content is `IdempotencyConflict`. The task state reflects
    /// later deliveries: see [`advance_task`] for which relay events move it.
    pub fn send_task(
        &mut self,
        id: &str,
        sender: &str,
        recipient: &str,
        body: &str,
    ) -> Result<TaskState, MessageError> {
        validate_message(id, sender, recipient, body)?;
        let now = chrono::Utc::now();
        let now_text = now.to_rfc3339();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<(String, String, String)> = tx
            .query_row(
                "SELECT sender, recipient, body FROM relay_messages WHERE sender=?1 AND id=?2",
                params![sender, id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if let Some((old_sender, old_recipient, old_body)) = existing {
            if old_sender != sender || old_recipient != recipient || old_body != body {
                return Err(MessageError::IdempotencyConflict);
            }
            // A message sent without a task (through `send`) has no task to report.
            let state = task_state_in(&tx, sender, id)?.ok_or(MessageError::IdempotencyConflict)?;
            tx.commit()?;
            return Ok(state);
        }
        // A task row with no message is left by an older send that failed between its two
        // writes. It is reused only while it is still `submitted` to the same recipient.
        let orphan: Option<(String, String)> = tx
            .query_row(
                "SELECT recipient, state FROM relay_tasks WHERE sender=?1 AND id=?2",
                params![sender, id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match orphan {
            Some((orphan_recipient, state))
                if orphan_recipient == recipient && state == "submitted" => {}
            Some(_) => return Err(MessageError::IdempotencyConflict),
            None => {
                tx.execute(
                    "INSERT INTO relay_tasks (id, sender, recipient, state, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 'submitted', ?4, ?4)",
                    params![id, sender, recipient, now_text],
                )?;
            }
        }
        tx.execute(
            "INSERT INTO relay_messages(id,sender,recipient,body,created_at) VALUES (?1,?2,?3,?4,?5)",
            params![id, sender, recipient, body, now_text],
        )?;
        tx.commit()?;
        Ok(TaskState::Submitted)
    }

    /// Moves a task to `next` when the transition is allowed; the rules are in `task.rs`.
    /// Leaving `input-required` for `canceled` or `failed` also drops the question, so the message
    /// is not stuck waiting for an answer. Resuming needs [`provide_input`], not this method.
    pub fn set_task_state(
        &mut self,
        sender: &str,
        id: &str,
        next: TaskState,
    ) -> Result<TaskState, MessageError> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = task_state_in(&tx, sender, id)?
            .ok_or_else(|| MessageError::TaskNotFound(id.to_owned()))?;
        if current == TaskState::InputRequired && next == TaskState::Working {
            return Err(MessageError::TaskTransition(
                "use provide_input to resume an input-required task".to_owned(),
            ));
        }
        let target = task::transition(current, next).map_err(MessageError::TaskTransition)?;
        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE relay_tasks SET state = ?1, updated_at = ?2 WHERE sender = ?3 AND id = ?4",
            rusqlite::params![target.as_str(), now, sender, id],
        )?;
        if current == TaskState::InputRequired {
            tx.execute(
                "UPDATE relay_messages SET awaiting_input = 0, input_prompt = NULL WHERE sender = ?1 AND id = ?2",
                params![sender, id],
            )?;
        }
        // A canceled or failed task has no further work, so its unacknowledged message is closed
        // as `gagal`. Otherwise it would stay claimable-looking in the inbox and `relay wait`.
        let reason = match target {
            TaskState::Canceled => Some("task canceled"),
            TaskState::Failed => Some("task failed"),
            _ => None,
        };
        if let Some(reason) = reason {
            close_pending_message(
                &tx,
                sender,
                id,
                reason,
                chrono::Utc::now().timestamp_millis(),
            )?;
        }
        tx.commit()?;
        Ok(target)
    }

    /// The recipient asks the sender for input and gives up its lease, so the task moves from
    /// `working` to `input-required`. The attempt is not counted: a question is not a failure.
    /// The message is not claimable until [`provide_input`] answers it. Requires the live lease
    /// `owner`; a stale or missing lease returns `LeaseConflict`. A task that cannot move to
    /// `input-required` (canceled, failed, completed) refuses the request, and the lease is kept.
    pub fn request_input(
        &mut self,
        sender: &str,
        id: &str,
        recipient: &str,
        owner: &str,
        question: &str,
    ) -> Result<(), MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        validate_agent(recipient)?;
        validate_id(owner)?;
        if question.is_empty() || question.len() > MAX_BODY_BYTES {
            return Err(MessageError::Invalid(
                "question must be 1..=65536 UTF-8 bytes",
            ));
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // Step 1: the message must belong to this recipient. Other recipients learn nothing about
        // the task, not even its state.
        if !message_belongs_to(&tx, sender, id, recipient)? {
            return Err(MessageError::LeaseConflict);
        }
        // Step 2: the task must be `working`. A message without a task row is refused as before.
        let current = task_state_in(&tx, sender, id)?
            .ok_or_else(|| MessageError::TaskNotFound(id.to_owned()))?;
        if current != TaskState::Working {
            return Err(MessageError::TaskTransition(format!(
                "task in state {current} cannot ask for input"
            )));
        }
        // Step 3: the lease. Any error below drops `tx` without a commit, so the lease is kept.
        let changed = tx.execute(
            "UPDATE relay_messages SET lease_owner=NULL,lease_expiry_epoch_ms=NULL,attempts=attempts-1,awaiting_input=1,input_prompt=?1,input_reply=NULL WHERE sender=?6 AND id=?2 AND recipient=?3 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND awaiting_input=0 AND lease_owner=?4 AND lease_expiry_epoch_ms>?5",
            params![question, id, recipient, owner, now_ms, sender],
        )?;
        if changed != 1 {
            return Err(MessageError::LeaseConflict);
        }
        task::transition(current, TaskState::InputRequired)
            .map_err(MessageError::TaskTransition)?;
        tx.execute(
            "UPDATE relay_tasks SET state='input-required', updated_at=?1 WHERE sender=?2 AND id=?3",
            params![chrono::Utc::now().to_rfc3339(), sender, id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The sender answers a task that is `input-required`. The task moves to `working`, and the
    /// message becomes claimable again. The answer is delivered in `input_reply` with the next claim.
    /// Any other state is refused with `TaskTransition`, so a canceled or finished task stays put.
    pub fn provide_input(
        &mut self,
        sender: &str,
        id: &str,
        reply: &str,
    ) -> Result<TaskState, MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        if reply.is_empty() || reply.len() > MAX_BODY_BYTES {
            return Err(MessageError::Invalid("reply must be 1..=65536 UTF-8 bytes"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current = task_state_in(&tx, sender, id)?
            .ok_or_else(|| MessageError::TaskNotFound(id.to_owned()))?;
        if current != TaskState::InputRequired {
            return Err(MessageError::TaskTransition(format!(
                "task in state {current} is not waiting for input"
            )));
        }
        let changed = tx.execute(
            "UPDATE relay_messages SET awaiting_input=0,input_reply=?1 WHERE sender=?2 AND id=?3 AND awaiting_input=1",
            params![reply, sender, id],
        )?;
        if changed != 1 {
            return Err(MessageError::TaskTransition(
                "task is input-required but its message is not awaiting input".to_owned(),
            ));
        }
        tx.execute(
            "UPDATE relay_tasks SET state='working', updated_at=?1 WHERE sender=?2 AND id=?3",
            params![chrono::Utc::now().to_rfc3339(), sender, id],
        )?;
        tx.commit()?;
        Ok(TaskState::Working)
    }

    /// State of task `(sender, id)`, with the question when it is `input-required`.
    pub fn task_view(&self, sender: &str, id: &str) -> Result<Option<TaskView>, MessageError> {
        let Some(state) = task_state_in(&self.connection, sender, id)? else {
            return Ok(None);
        };
        let prompt = if state == TaskState::InputRequired {
            self.connection
                .query_row(
                    "SELECT input_prompt FROM relay_messages WHERE sender=?1 AND id=?2",
                    params![sender, id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .flatten()
        } else {
            None
        };
        Ok(Some(TaskView { state, prompt }))
    }

    /// Enqueues a message; reusing `id` with identical content is idempotent.
    pub fn send(
        &mut self,
        id: &str,
        sender: &str,
        recipient: &str,
        body: &str,
    ) -> Result<Message, MessageError> {
        validate_message(id, sender, recipient, body)?;
        let now = chrono::Utc::now();
        let created_at = now.to_rfc3339();
        let now_ms = now.timestamp_millis();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing: Option<Message> = tx
            .query_row(
                &format!("{} WHERE sender=?1 AND id=?2", select_messages(now_ms)),
                params![sender, id],
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
            input_prompt: None,
            input_reply: None,
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

    /// Returns one message by its sender and ID with its current status, or `None` when it does
    /// not exist. IDs are unique per sender only, so the sender is always part of the key.
    pub fn find(&self, sender: &str, id: &str) -> Result<Option<Message>, MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let message = self
            .connection
            .query_row(
                &format!("{} WHERE sender=?1 AND id=?2", select_messages(now_ms)),
                params![sender, id],
                row_to_message,
            )
            .optional()?;
        Ok(message)
    }

    /// Returns the message `(sender, id)` only when it is addressed to `recipient`, so a caller
    /// cannot read or probe messages of other recipients.
    pub fn find_for_recipient(
        &self,
        recipient: &str,
        sender: &str,
        id: &str,
    ) -> Result<Option<Message>, MessageError> {
        validate_agent(recipient)?;
        let message = self.find(sender, id)?;
        Ok(message.filter(|message| message.recipient == recipient))
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
        // A message whose task is canceled, failed, completed, or waiting for input is skipped.
        // A message with no task row (from `send`) is still claimable.
        let next: Option<(String, String)> = tx
            .query_row(
                &format!(
                    "SELECT m.id, m.sender FROM relay_messages m \
                     LEFT JOIN relay_tasks t ON t.sender = m.sender AND t.id = m.id \
                     WHERE m.recipient=?1 AND m.acknowledged_at IS NULL AND m.failed_at_epoch_ms IS NULL \
                     AND m.awaiting_input=0 AND m.attempts<{MAX_ATTEMPTS} \
                     AND (m.lease_expiry_epoch_ms IS NULL OR m.lease_expiry_epoch_ms<=?2) \
                     AND (t.state IS NULL OR t.state IN ('submitted','working')) \
                     ORDER BY m.created_at,m.id LIMIT 1"
                ),
                params![recipient, now_ms],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((id, sender)) = next else {
            tx.commit()?;
            return Ok(None);
        };
        let changed = tx.execute(
            &format!(
                "UPDATE relay_messages SET lease_owner=?1,lease_expiry_epoch_ms=?2,attempts=attempts+1 WHERE sender=?6 AND id=?3 AND recipient=?4 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND awaiting_input=0 AND attempts<{MAX_ATTEMPTS} AND (lease_expiry_epoch_ms IS NULL OR lease_expiry_epoch_ms<=?5)"
            ),
            params![token, expiry_ms, id, recipient, now_ms, sender],
        )?;
        if changed != 1 {
            return Err(MessageError::LeaseConflict);
        }
        let message = tx.query_row(
            &format!("{} WHERE sender=?1 AND id=?2", select_messages(now_ms)),
            params![sender, id],
            row_to_message,
        )?;
        advance_task(
            &tx,
            &message.sender,
            &id,
            TaskState::Working,
            &chrono::Utc::now().to_rfc3339(),
        )?;
        tx.commit()?;
        Ok(Some(message))
    }

    /// Acknowledge a claimed message only when the caller still owns its lease.
    pub fn acknowledge_claim(
        &mut self,
        sender: &str,
        id: &str,
        recipient: &str,
        owner: &str,
    ) -> Result<bool, MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        validate_agent(recipient)?;
        validate_id(owner)?;
        let now = chrono::Utc::now();
        let now_text = now.to_rfc3339();
        let now_ms = now.timestamp_millis();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // A message that is not this recipient's is reported as not acknowledged, with no state.
        if !message_belongs_to(&tx, sender, id, recipient)? {
            return Ok(false);
        }
        // Task state is checked before the lease, so a task that is no longer `working` is
        // refused with `TaskTransition` even when the lease is still held.
        require_working(&tx, sender, id)?;
        let changed = tx.execute(
            "UPDATE relay_messages SET acknowledged_at=?1,acked_at_epoch_ms=?2,lease_owner=NULL,lease_expiry_epoch_ms=NULL WHERE sender=?7 AND id=?3 AND recipient=?4 AND lease_owner=?5 AND lease_expiry_epoch_ms>?6 AND acknowledged_at IS NULL",
            params![now_text, now_ms, id, recipient, owner, now_ms, sender],
        )?;
        if changed == 1 {
            complete_task(&tx, sender, id, &now_text)?;
        }
        tx.commit()?;
        Ok(changed == 1)
    }

    /// Releases a claim after a failed attempt. The message returns to `menunggu` for
    /// another attempt, or becomes `gagal` when its last allowed attempt was used.
    /// Returns the resulting status, or `None` when the lease token is stale.
    pub fn release_claim(
        &mut self,
        sender: &str,
        id: &str,
        recipient: &str,
        owner: &str,
        error: &str,
    ) -> Result<Option<MessageStatus>, MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        validate_agent(recipient)?;
        validate_id(owner)?;
        if error.len() > MAX_ERROR_BYTES {
            return Err(MessageError::Invalid(
                "error must be at most 1024 UTF-8 bytes",
            ));
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if !message_belongs_to(&tx, sender, id, recipient)? {
            return Ok(None);
        }
        require_working(&tx, sender, id)?;
        let changed = tx.execute(
            &format!(
                "UPDATE relay_messages SET lease_owner=NULL,lease_expiry_epoch_ms=NULL,last_error=?1,failed_at_epoch_ms=CASE WHEN attempts>={MAX_ATTEMPTS} THEN ?2 ELSE NULL END WHERE sender=?6 AND id=?3 AND recipient=?4 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND lease_owner=?5 AND lease_expiry_epoch_ms>?2"
            ),
            params![error, now_ms, id, recipient, owner, sender],
        )?;
        if changed != 1 {
            tx.commit()?;
            return Ok(None);
        }
        let failed: bool = tx.query_row(
            "SELECT failed_at_epoch_ms IS NOT NULL FROM relay_messages WHERE sender=?1 AND id=?2",
            params![sender, id],
            |row| row.get(0),
        )?;
        if failed {
            advance_task(
                &tx,
                sender,
                id,
                TaskState::Failed,
                &chrono::Utc::now().to_rfc3339(),
            )?;
        }
        tx.commit()?;
        Ok(Some(if failed {
            MessageStatus::Failed
        } else {
            MessageStatus::Pending
        }))
    }

    /// Acknowledge only never-claimed messages for legacy callers; lease tokens cannot be bypassed.
    pub fn acknowledge(
        &mut self,
        sender: &str,
        id: &str,
        recipient: &str,
    ) -> Result<bool, MessageError> {
        validate_id(id)?;
        validate_agent(sender)?;
        validate_agent(recipient)?;
        let now = chrono::Utc::now();
        let now_text = now.to_rfc3339();
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // A message closed as `gagal` (its task was canceled or failed) is never acknowledged here.
        let changed = tx.execute(
            "UPDATE relay_messages SET acknowledged_at=?1,acked_at_epoch_ms=?2 WHERE sender=?5 AND id=?3 AND recipient=?4 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND attempts=0 AND awaiting_input=0",
            params![now_text, now.timestamp_millis(), id, recipient, sender],
        )?;
        if changed == 1 {
            complete_task(&tx, sender, id, &now_text)?;
        }
        tx.commit()?;
        Ok(changed == 1)
    }

    /// Deletes done messages whose acknowledgment is older than `older_than`.
    /// Failed and unfinished messages are kept. A purged ID is no longer deduplicated,
    /// so choose a retention period longer than any sender's retry window.
    pub fn purge_completed(&mut self, older_than: Duration) -> Result<usize, MessageError> {
        let age_ms = i64::try_from(older_than.as_millis()).unwrap_or(i64::MAX);
        let cutoff_ms = chrono::Utc::now().timestamp_millis().saturating_sub(age_ms);
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
    let legacy: Vec<(String, String, String)> = connection
        .prepare(
            "SELECT sender,id,acknowledged_at FROM relay_messages WHERE acknowledged_at IS NOT NULL AND acked_at_epoch_ms IS NULL",
        )?
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<(String, String, String)>, rusqlite::Error>>()?;
    for (sender, id, acknowledged_at) in legacy {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(&acknowledged_at) {
            connection.execute(
                "UPDATE relay_messages SET acked_at_epoch_ms=?1 WHERE sender=?2 AND id=?3",
                params![parsed.timestamp_millis(), sender, id],
            )?;
        }
    }
    Ok(())
}

/// Moves the primary key of `relay_messages` from `(id)` to `(sender, id)`, which is needed for
/// IDs to be unique per sender. SQLite cannot alter a key in place, so the table is rebuilt in one
/// transaction: rename, create with the new key, copy every row, drop the old table, and recreate
/// the inbox index. Runs after `migrate_columns`, so every column is present. A table that already
/// has the composite key is left alone.
fn migrate_primary_key(connection: &mut Connection) -> Result<(), MessageError> {
    let key_columns: Vec<(String, i64)> = connection
        .prepare("PRAGMA table_info(relay_messages)")?
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;
    let sender_position = key_columns
        .iter()
        .find(|(name, _)| name == "sender")
        .map(|c| c.1);
    let id_position = key_columns
        .iter()
        .find(|(name, _)| name == "id")
        .map(|c| c.1);
    if sender_position == Some(1) && id_position == Some(2) {
        return Ok(());
    }
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "DROP INDEX IF EXISTS relay_messages_inbox;
         ALTER TABLE relay_messages RENAME TO relay_messages_old;
         CREATE TABLE relay_messages (
           id TEXT NOT NULL,
           sender TEXT NOT NULL,
           recipient TEXT NOT NULL,
           body TEXT NOT NULL,
           created_at TEXT NOT NULL,
           acknowledged_at TEXT,
           lease_owner TEXT,
           lease_expiry_epoch_ms INTEGER,
           attempts INTEGER NOT NULL DEFAULT 0,
           failed_at_epoch_ms INTEGER,
           acked_at_epoch_ms INTEGER,
           last_error TEXT,
           awaiting_input INTEGER NOT NULL DEFAULT 0,
           input_prompt TEXT,
           input_reply TEXT,
           PRIMARY KEY(sender, id)
         );
         INSERT INTO relay_messages (
           id, sender, recipient, body, created_at, acknowledged_at, lease_owner,
           lease_expiry_epoch_ms, attempts, failed_at_epoch_ms, acked_at_epoch_ms,
           last_error, awaiting_input, input_prompt, input_reply
         ) SELECT
           id, sender, recipient, body, created_at, acknowledged_at, lease_owner,
           lease_expiry_epoch_ms, attempts, failed_at_epoch_ms, acked_at_epoch_ms,
           last_error, awaiting_input, input_prompt, input_reply
         FROM relay_messages_old;
         DROP TABLE relay_messages_old;
         CREATE INDEX relay_messages_inbox ON relay_messages(recipient, acknowledged_at, created_at);",
    )?;
    tx.commit()?;
    Ok(())
}

/// SQL expression that derives the status of a row at `now_ms`.
fn status_sql(now_ms: i64) -> String {
    format!(
        "CASE WHEN acknowledged_at IS NOT NULL THEN 'selesai' \
         WHEN failed_at_epoch_ms IS NOT NULL THEN 'gagal' \
         WHEN awaiting_input=1 THEN 'menunggu-input' \
         WHEN lease_owner IS NOT NULL AND lease_expiry_epoch_ms>{now_ms} THEN 'diambil' \
         WHEN attempts>={MAX_ATTEMPTS} THEN 'gagal' \
         ELSE 'menunggu' END"
    )
}

/// SELECT clause that returns every `Message` column plus the derived status.
fn select_messages(now_ms: i64) -> String {
    format!(
        "SELECT id,sender,recipient,body,created_at,acknowledged_at,lease_owner,lease_expiry_epoch_ms,{status},attempts,last_error,input_prompt,input_reply FROM relay_messages",
        status = status_sql(now_ms)
    )
}

/// Filter for messages that are neither done nor failed at `now_ms`.
fn live_filter(now_ms: i64) -> String {
    format!(
        "acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL AND (attempts<{MAX_ATTEMPTS} OR lease_expiry_epoch_ms>{now_ms})"
    )
}

fn validate_message(
    id: &str,
    sender: &str,
    recipient: &str,
    body: &str,
) -> Result<(), MessageError> {
    validate_id(id)?;
    validate_agent(sender)?;
    validate_agent(recipient)?;
    if body.is_empty() || body.len() > MAX_BODY_BYTES {
        return Err(MessageError::Invalid("body must be 1..=65536 UTF-8 bytes"));
    }
    Ok(())
}

/// Reads the state of task `(sender, id)` through `conn`, which may be a transaction.
fn task_state_in(
    conn: &Connection,
    sender: &str,
    id: &str,
) -> Result<Option<TaskState>, rusqlite::Error> {
    let text: Option<String> = conn
        .query_row(
            "SELECT state FROM relay_tasks WHERE sender = ?1 AND id = ?2",
            params![sender, id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(text.and_then(|value| TaskState::parse(&value)))
}

/// Moves task `(sender, id)` to `next` when the state machine allows it, inside the caller's
/// transaction. A missing task, or a move the machine refuses (already past it, or canceled), leaves
/// the task as it is. The message is the authority for delivery, so a task move never fails or
/// undoes a message operation.
///
/// Events: a claim moves the task to `working`; a leased or legacy ack moves it to `completed` (via
/// `working` when it was still `submitted`); the final failed release moves it to `failed`. A
/// non-final release keeps `working`, because the state machine has no way back to `submitted`.
fn advance_task(
    conn: &Connection,
    sender: &str,
    id: &str,
    next: TaskState,
    now: &str,
) -> Result<(), rusqlite::Error> {
    let Some(current) = task_state_in(conn, sender, id)? else {
        return Ok(());
    };
    let Ok(target) = task::transition(current, next) else {
        return Ok(());
    };
    conn.execute(
        "UPDATE relay_tasks SET state=?1, updated_at=?2 WHERE sender=?3 AND id=?4",
        params![target.as_str(), now, sender, id],
    )?;
    Ok(())
}

/// Completes the task of an acknowledged message. Acknowledging implies delivery, so a task that is
/// still `submitted` passes through `working` first.
fn complete_task(
    conn: &Connection,
    sender: &str,
    id: &str,
    now: &str,
) -> Result<(), rusqlite::Error> {
    advance_task(conn, sender, id, TaskState::Working, now)?;
    advance_task(conn, sender, id, TaskState::Completed, now)
}

/// True when message `(sender, id)` exists and is addressed to `recipient`.
fn message_belongs_to(
    conn: &Connection,
    sender: &str,
    id: &str,
    recipient: &str,
) -> Result<bool, rusqlite::Error> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM relay_messages WHERE sender=?1 AND id=?2 AND recipient=?3",
            params![sender, id, recipient],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Refuses a task that is not `working`. A message without a task row has no state to check.
fn require_working(conn: &Connection, sender: &str, id: &str) -> Result<(), MessageError> {
    match task_state_in(conn, sender, id)? {
        Some(state) if state != TaskState::Working => Err(MessageError::TaskTransition(format!(
            "task in state {state} is not working"
        ))),
        _ => Ok(()),
    }
}

/// Closes an unfinished message of a task that was canceled or failed: it becomes `gagal`, its
/// lease and any pending question are dropped, and it leaves the inbox.
fn close_pending_message(
    conn: &Connection,
    sender: &str,
    id: &str,
    reason: &str,
    now_ms: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE relay_messages SET failed_at_epoch_ms=?1,last_error=?2,lease_owner=NULL,lease_expiry_epoch_ms=NULL,awaiting_input=0,input_prompt=NULL WHERE sender=?3 AND id=?4 AND acknowledged_at IS NULL AND failed_at_epoch_ms IS NULL",
        params![now_ms, reason, sender, id],
    )?;
    Ok(())
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
        "menunggu-input" => Ok(MessageStatus::AwaitingInput),
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
        input_prompt: row.get(11)?,
        input_reply: row.get(12)?,
    })
}

#[cfg(test)]
#[path = "relay_test.rs"]
mod relay_test;
