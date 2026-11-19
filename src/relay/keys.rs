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
}

impl KeySource for StaticKeySource {
    fn key(&self, key_id: &str) -> Option<Vec<u8>> {
        self.keys.get(key_id).cloned()
    }
}

#[cfg(test)]
#[path = "keys_test.rs"]
mod keys_test;
