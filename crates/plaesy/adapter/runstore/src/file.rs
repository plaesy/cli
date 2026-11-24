//! File-backed run log: the adapter that implements `plaesy_domain_workflow::runstore::RunSink`.
//!
//! Line format: `<16 hex chars of sha256(payload)> <payload>\n`. Each append is
//! synced to disk before it returns. A final fragment without a newline is a torn
//! tail: it is reported and never returned as a record. A checksum mismatch on a
//! complete line is corruption and is an error, never silently dropped.

use plaesy_domain_workflow::runstore::{Recovered, RunLogError, RunSink};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

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

    /// Appends one record and syncs it to disk before returning (see [`RunSink::append`]).
    ///
    /// # Errors
    ///
    /// Returns [`RunLogError::InvalidPayload`] for payloads with a newline, or an I/O error.
    fn append_line(&self, payload: &str) -> Result<(), RunLogError> {
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
    fn recover_lines(&self) -> Result<Recovered, RunLogError> {
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

impl RunSink for RunLog {
    fn append(&self, payload: &str) -> Result<(), RunLogError> {
        self.append_line(payload)
    }

    fn recover(&self) -> Result<Recovered, RunLogError> {
        self.recover_lines()
    }
}

#[cfg(test)]
#[path = "file_test.rs"]
mod file_test;
