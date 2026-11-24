//! Durable append-only run log (spec VS-09, VS-10): the port and its error types.
//!
//! The file-backed implementation lives in `plaesy-adapter-runstore`. This module
//! has no I/O, so the domain stays free of file and hash dependencies.

use std::sync::{Arc, Mutex};

pub mod audit;
pub mod drain;

/// Errors from the run log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogError {
    /// The payload contains a newline and cannot be stored as one line.
    InvalidPayload,
    /// Reading or writing the storage failed.
    Io(String),
    /// A complete line does not match its checksum (1-based line number).
    Corrupt { line: usize },
}

impl std::fmt::Display for RunLogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPayload => write!(f, "payload must not contain a newline"),
            Self::Io(message) => write!(f, "run log io: {message}"),
            Self::Corrupt { line } => write!(f, "run log corrupt at line {line}"),
        }
    }
}

impl std::error::Error for RunLogError {}

/// Result of reading a log back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovered {
    /// Payloads of all complete, verified records, in order.
    pub records: Vec<String>,
    /// True when the storage ends in an incomplete record that was ignored.
    pub torn_tail: bool,
}

/// Storage port for the run log. Implementations must sync each append before it returns.
pub trait RunSink {
    /// Appends one record.
    ///
    /// # Errors
    ///
    /// Returns [`RunLogError::InvalidPayload`] for payloads with a newline, or a storage error.
    fn append(&self, payload: &str) -> Result<(), RunLogError>;

    /// Reads back all verified records.
    ///
    /// # Errors
    ///
    /// Returns [`RunLogError::Corrupt`] when a complete record fails verification.
    fn recover(&self) -> Result<Recovered, RunLogError>;
}

/// In-memory sink for tests and dry runs. Clones share the same records.
#[derive(Debug, Clone, Default)]
pub struct MemoryLog {
    records: Arc<Mutex<Vec<String>>>,
}

impl RunSink for MemoryLog {
    fn append(&self, payload: &str) -> Result<(), RunLogError> {
        if payload.contains('\n') {
            return Err(RunLogError::InvalidPayload);
        }
        let mut records = self
            .records
            .lock()
            .map_err(|_| RunLogError::Io("memory log poisoned".to_owned()))?;
        records.push(payload.to_owned());
        Ok(())
    }

    fn recover(&self) -> Result<Recovered, RunLogError> {
        let records = self
            .records
            .lock()
            .map_err(|_| RunLogError::Io("memory log poisoned".to_owned()))?;
        Ok(Recovered {
            records: records.clone(),
            torn_tail: false,
        })
    }
}
