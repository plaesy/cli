//! Key lookup for message envelopes (spec R6 key distribution, best-practice default).
//!
//! Default for deployments: keys live in the OS credential store under one service
//! name, one entry per key id, so they are never in the repository, in envelopes,
//! or in logs. Rotation means adding a new key id and retiring the old one.

use std::collections::HashMap;

/// Source of HMAC keys by key id.
pub trait KeySource {
    /// Returns the key bytes for `key_id`, or `None` when the id is unknown.
    fn key(&self, key_id: &str) -> Option<Vec<u8>>;
}

/// Reads keys from the OS credential store (Windows Credential Manager, macOS Keychain, Secret Service).
#[derive(Debug, Clone)]
pub struct KeyringKeySource {
    service: String,
}

impl KeyringKeySource {
    /// Creates a source that reads entries under `service`.
    #[must_use]
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl KeySource for KeyringKeySource {
    fn key(&self, key_id: &str) -> Option<Vec<u8>> {
        keyring::Entry::new(&self.service, key_id)
            .ok()?
            .get_password()
            .ok()
            .map(String::into_bytes)
    }
}

/// In-memory keys for tests. Do not use for deployment keys.
#[derive(Debug, Default)]
pub struct StaticKeySource {
    keys: HashMap<String, Vec<u8>>,
}

impl StaticKeySource {
    /// Creates an empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a key.
    pub fn insert(&mut self, key_id: &str, key: &[u8]) {
        self.keys.insert(key_id.to_owned(), key.to_vec());
    }

    /// Retires a key. Envelopes signed with it are refused from now on.
    pub fn remove(&mut self, key_id: &str) {
        self.keys.remove(key_id);
    }
}

impl KeySource for StaticKeySource {
    fn key(&self, key_id: &str) -> Option<Vec<u8>> {
        self.keys.get(key_id).cloned()
    }
}

/// Reads keys from one file per key id in a directory. Intended for systemd `LoadCredential=`,
/// where the directory is `$CREDENTIALS_DIRECTORY`. Headless servers without a secret service use this.
#[derive(Debug, Clone)]
pub struct FileKeySource {
    dir: std::path::PathBuf,
}

impl FileKeySource {
    /// Creates a source that reads `<dir>/<key_id>`.
    #[must_use]
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

impl KeySource for FileKeySource {
    fn key(&self, key_id: &str) -> Option<Vec<u8>> {
        let safe = !key_id.is_empty()
            && !key_id.starts_with('.')
            && key_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !safe {
            return None;
        }
        let mut bytes = std::fs::read(self.dir.join(key_id)).ok()?;
        while bytes.last().is_some_and(u8::is_ascii_whitespace) {
            bytes.pop();
        }
        if bytes.is_empty() {
            None
        } else {
            Some(bytes)
        }
    }
}

#[cfg(test)]
#[path = "keys_test.rs"]
mod keys_test;
