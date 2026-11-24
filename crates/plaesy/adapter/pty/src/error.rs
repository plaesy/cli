use std::io;

use thiserror::Error;

use crate::table::SessionId;

/// Galat yang dikembalikan `PtyManager` dan tipe pendukungnya.
#[derive(Debug, Error)]
pub enum PtyError {
    #[error("invalid command: {0}")]
    InvalidCommand(&'static str),
    #[error("terminal size must be 1..=1000 rows and columns, got {rows}x{cols}")]
    InvalidSize { rows: u16, cols: u16 },
    #[error("invalid pty config: {0}")]
    InvalidConfig(&'static str),
    #[error("session limit reached ({limit} open sessions)")]
    SessionLimitReached { limit: usize },
    #[error("session {0} not found")]
    SessionNotFound(SessionId),
    #[error("session registry lock was poisoned by a panic")]
    Poisoned,
    #[error("pty backend failed to {operation}: {message}")]
    Backend {
        operation: &'static str,
        message: String,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
}
