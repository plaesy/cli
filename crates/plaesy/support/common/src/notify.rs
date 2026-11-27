//! Durable, project-local notifications in `.plaesy/notifications.jsonl`.
//!
//! Each line is one notification. Writes append under a file lock. Marking as read stamps
//! `read_at` and keeps the record. Only `remove` deletes records. Nothing here is sent off the
//! machine.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const FILE: &str = "notifications.jsonl";
const LOCK: &str = ".notifications.lock";
const MAX_MESSAGE_CHARS: usize = 500;
const MAX_SESSION_CHARS: usize = 128;

/// The four event kinds a notification can describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A session is waiting for a decision or input from a person.
    InputNeeded,
    /// A pull request is ready to merge.
    ReadyToMerge,
    /// A pull request was merged.
    Merged,
    /// A pull request was closed without merging.
    Closed,
}

impl Kind {
    /// Every kind, in the order they are documented.
    pub const ALL: [Self; 4] = [
        Self::InputNeeded,
        Self::ReadyToMerge,
        Self::Merged,
        Self::Closed,
    ];

    /// The kebab-case name used on the command line and in the store.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InputNeeded => "input-needed",
            Self::ReadyToMerge => "ready-to-merge",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }

    /// Parses a kind name, returning `None` for anything unknown.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }
}

/// One stored notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub kind: Kind,
    pub session: String,
    pub message: String,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<u64>,
}

/// Which records a `mark_read` or `remove` call applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    One(&'a str),
    All,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid notification: {0}")]
    Invalid(String),
    #[error("notification not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn validate(session: &str, message: &str) -> Result<(), Error> {
    let session = session.trim();
    if session.is_empty() || session.chars().count() > MAX_SESSION_CHARS {
        return Err(Error::Invalid(format!(
            "session must be 1..={MAX_SESSION_CHARS} characters"
        )));
    }
    if session.chars().any(char::is_control) {
        return Err(Error::Invalid(
            "session must not contain control characters".into(),
        ));
    }
    if message.trim().is_empty() || message.chars().count() > MAX_MESSAGE_CHARS {
        return Err(Error::Invalid(format!(
            "message must be 1..={MAX_MESSAGE_CHARS} characters"
        )));
    }
    if message.chars().any(|c| c.is_control() && c != ' ') {
        return Err(Error::Invalid(
            "message must be a single line of printable text".into(),
        ));
    }
    Ok(())
}

/// Runs `f` while holding the store lock. The lock file lives beside the store.
fn locked<T>(root: &Path, f: impl FnOnce(&Path) -> Result<T, Error>) -> Result<T, Error> {
    let dir = root.join(".plaesy");
    fs::create_dir_all(&dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(LOCK))?;
    lock.lock()?;
    let result = f(&dir);
    let _ = lock.unlock();
    result
}

fn read(dir: &Path) -> Result<Vec<Notification>, Error> {
    let path = dir.join(FILE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Error::from))
        .collect()
}

/// Replaces the whole store through a temporary file, so a crash never leaves a half-written
/// store behind.
fn write(dir: &Path, list: &[Notification]) -> Result<(), Error> {
    let mut text = String::new();
    for item in list {
        text.push_str(&serde_json::to_string(item)?);
        text.push('\n');
    }
    let tmp = dir.join(format!(".notifications-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, text)?;
    fs::rename(tmp, dir.join(FILE))?;
    Ok(())
}

/// Appends a notification and returns the stored record.
pub fn add(root: &Path, kind: Kind, session: &str, message: &str) -> Result<Notification, Error> {
    validate(session, message)?;
    let item = Notification {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        session: session.trim().to_owned(),
        message: message.trim().to_owned(),
        created_at: now(),
        read_at: None,
    };
    locked(root, |dir| {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(FILE))?;
        writeln!(file, "{}", serde_json::to_string(&item)?)?;
        Ok(())
    })?;
    Ok(item)
}

/// Returns every notification, oldest first.
pub fn list(root: &Path) -> Result<Vec<Notification>, Error> {
    locked(root, read)
}

/// Stamps `read_at` on the selected unread records and returns how many changed. The records are
/// kept. A `One` target that does not exist is an error.
pub fn mark_read(root: &Path, target: Target<'_>) -> Result<usize, Error> {
    locked(root, |dir| {
        let mut list = read(dir)?;
        if let Target::One(id) = target {
            if !list.iter().any(|item| item.id == id) {
                return Err(Error::NotFound(id.to_owned()));
            }
        }
        let stamp = now();
        let mut changed = 0;
        for item in list.iter_mut().filter(|item| selected(target, &item.id)) {
            if item.read_at.is_none() {
                item.read_at = Some(stamp);
                changed += 1;
            }
        }
        if changed > 0 {
            write(dir, &list)?;
        }
        Ok(changed)
    })
}

/// Deletes the selected records and returns how many were removed. A `One` target that does not
/// exist is an error.
pub fn remove(root: &Path, target: Target<'_>) -> Result<usize, Error> {
    locked(root, |dir| {
        let list = read(dir)?;
        if let Target::One(id) = target {
            if !list.iter().any(|item| item.id == id) {
                return Err(Error::NotFound(id.to_owned()));
            }
        }
        let before = list.len();
        let kept: Vec<Notification> = list
            .into_iter()
            .filter(|item| !selected(target, &item.id))
            .collect();
        let removed = before - kept.len();
        if removed > 0 {
            write(dir, &kept)?;
        }
        Ok(removed)
    })
}

fn selected(target: Target<'_>, id: &str) -> bool {
    match target {
        Target::One(one) => one == id,
        Target::All => true,
    }
}

#[cfg(test)]
#[path = "notify_test.rs"]
mod notify_test;
