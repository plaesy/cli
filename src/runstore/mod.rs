//! Durable append-only run log with checksummed lines (spec VS-09, VS-10).
//!
//! Line format: `<16 hex chars of sha256(payload)> <payload>\n`. Each append is
//! synced to disk before it returns. A final fragment without a newline is a torn
//! tail: it is reported and never returned as a record. A checksum mismatch on a
//! complete line is corruption and is an error, never silently dropped.

use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

pub mod drain;

/// Errors from the run log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogError {
    /// The payload contains a newline and cannot be stored as one line.
    InvalidPayload,
    /// Reading or writing the file failed.
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
    /// Payloads of all complete, verified lines, in order.
    pub records: Vec<String>,
    /// True when the file ends in an incomplete line that was ignored.
    pub torn_tail: bool,
}

/// Checksum of one payload: the first 8 bytes of its SHA-256, as hex.
#[must_use]
pub fn checksum(payload: &str) -> String {
    Sha256::digest(payload.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn io_error(error: &std::io::Error) -> RunLogError {
    RunLogError::Io(error.to_string())
}

/// Append-only log stored in one file.
#[derive(Debug, Clone)]
pub struct RunLog {
    path: PathBuf,
}

impl RunLog {
    /// Opens a log at `path`. The file is created on first append.
    #[must_use]
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Appends one record and syncs it to disk before returning.
    ///
    /// # Errors
    ///
    /// Returns [`RunLogError::InvalidPayload`] for payloads with a newline, or an I/O error.
    pub fn append(&self, payload: &str) -> Result<(), RunLogError> {
        if payload.contains('\n') {
            return Err(RunLogError::InvalidPayload);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| io_error(&error))?;
        writeln!(file, "{} {}", checksum(payload), payload).map_err(|error| io_error(&error))?;
        file.sync_data().map_err(|error| io_error(&error))
    }

    /// Reads back all verified records and reports a torn tail if one exists.
    ///
    /// # Errors
    ///
    /// Returns [`RunLogError::Corrupt`] when a complete line fails its checksum.
    pub fn recover(&self) -> Result<Recovered, RunLogError> {
        if !self.path.exists() {
            return Ok(Recovered {
                records: Vec::new(),
                torn_tail: false,
            });
        }
        let mut text = String::new();
        File::open(&self.path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .map_err(|error| io_error(&error))?;

        let mut lines: Vec<&str> = text.split('\n').collect();
        // The last element is empty when the file ends with a newline. Otherwise it is a torn fragment.
        let tail = lines.pop().unwrap_or("");
        let torn_tail = !tail.is_empty();

        let mut records = Vec::with_capacity(lines.len());
        for (index, line) in lines.iter().enumerate() {
            let corrupt = RunLogError::Corrupt { line: index + 1 };
            let (sum, payload) = line.split_once(' ').ok_or_else(|| corrupt.clone())?;
            if sum != checksum(payload) {
                return Err(corrupt);
            }
            records.push(payload.to_owned());
        }
        Ok(Recovered { records, torn_tail })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runstore_round_trip_keeps_order() {
        let dir = tempfile::tempdir().unwrap();
        let log = RunLog::open(dir.path().join("run.log"));
        log.append("first").unwrap();
        log.append("second").unwrap();
        let recovered = log.recover().unwrap();
        assert_eq!(recovered.records, vec!["first", "second"]);
        assert!(!recovered.torn_tail);
    }

    #[test]
    fn runstore_torn_tail_is_reported_and_not_a_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.log");
        let log = RunLog::open(&path);
        log.append("complete-record").unwrap();
        let mut raw = OpenOptions::new().append(true).open(&path).unwrap();
        raw.write_all(b"deadbeef half-written").unwrap();
        let recovered = log.recover().unwrap();
        assert_eq!(recovered.records, vec!["complete-record"]);
        assert!(recovered.torn_tail);
    }

    #[test]
    fn runstore_corruption_before_tail_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.log");
        let log = RunLog::open(&path);
        log.append("good").unwrap();
        let mut raw = OpenOptions::new().append(true).open(&path).unwrap();
        raw.write_all(b"0000000000000000 tampered\n").unwrap();
        log.append("after").unwrap();
        assert_eq!(log.recover(), Err(RunLogError::Corrupt { line: 2 }));
    }

    #[test]
    fn runstore_rejects_payload_with_newline() {
        let dir = tempfile::tempdir().unwrap();
        let log = RunLog::open(dir.path().join("run.log"));
        assert_eq!(log.append("a\nb"), Err(RunLogError::InvalidPayload));
    }
}
