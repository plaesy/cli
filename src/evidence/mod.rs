//! Content-addressed evidence store (spec R3, VS-02, VS-16).
//!
//! An artifact is stored under the SHA-256 of its bytes. A success is only
//! trustworthy when the evidence reference points at stored bytes whose hash
//! still matches. Reads re-verify the hash, so tampering is reported, not served.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// Reference to one stored artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRef {
    /// Lowercase hex SHA-256 of the artifact bytes.
    pub sha256: String,
}

/// Errors from the evidence store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    /// No artifact with this hash exists.
    Missing,
    /// The stored bytes no longer match their hash.
    HashMismatch,
    /// The reference is not a valid hash.
    InvalidReference,
    /// Reading or writing failed.
    Io(String),
}

/// Directory-backed store.
#[derive(Debug, Clone)]
pub struct EvidenceStore {
    root: PathBuf,
}

fn hash_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl EvidenceStore {
    /// Opens a store rooted at `root`. The directory is created on first write.
    #[must_use]
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Stores bytes and returns their reference.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError::Io`] when the directory or file cannot be written.
    pub fn put(&self, bytes: &[u8]) -> Result<EvidenceRef, EvidenceError> {
        fs::create_dir_all(&self.root).map_err(|error| EvidenceError::Io(error.to_string()))?;
        let sha256 = hash_hex(bytes);
        let path = self.root.join(&sha256);
        if !path.exists() {
            fs::write(&path, bytes).map_err(|error| EvidenceError::Io(error.to_string()))?;
        }
        Ok(EvidenceRef { sha256 })
    }

    /// Reads bytes back and verifies them against the reference.
    ///
    /// # Errors
    ///
    /// Returns [`EvidenceError::HashMismatch`] when the stored bytes were changed.
    pub fn get(&self, evidence: &EvidenceRef) -> Result<Vec<u8>, EvidenceError> {
        if evidence.sha256.len() != 64 || !evidence.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(EvidenceError::InvalidReference);
        }
        let path = self.root.join(&evidence.sha256);
        let bytes = fs::read(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => EvidenceError::Missing,
            _ => EvidenceError::Io(error.to_string()),
        })?;
        if hash_hex(&bytes) != evidence.sha256 {
            return Err(EvidenceError::HashMismatch);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_run_evidence_round_trip_verifies_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = EvidenceStore::open(dir.path().join("evidence"));
        let reference = store.put(b"report body").unwrap();
        assert_eq!(store.get(&reference).unwrap(), b"report body");
    }

    #[test]
    fn task_run_evidence_tampered_bytes_are_not_served() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("evidence");
        let store = EvidenceStore::open(&root);
        let reference = store.put(b"report body").unwrap();
        fs::write(root.join(&reference.sha256), b"changed").unwrap();
        assert_eq!(store.get(&reference), Err(EvidenceError::HashMismatch));
    }

    #[test]
    fn task_run_evidence_missing_and_invalid_references_fail() {
        let dir = tempfile::tempdir().unwrap();
        let store = EvidenceStore::open(dir.path().join("evidence"));
        let missing = EvidenceRef {
            sha256: "a".repeat(64),
        };
        assert_eq!(store.get(&missing), Err(EvidenceError::Missing));
        let invalid = EvidenceRef {
            sha256: "not-a-hash".into(),
        };
        assert_eq!(store.get(&invalid), Err(EvidenceError::InvalidReference));
    }
}
