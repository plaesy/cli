//! Secret references and redaction (spec R9, VS-15).
//!
//! Tasks refer to secrets by name. Raw values stay in the store. Anything written
//! to events, logs, or exports passes through [`SecretStore::redact`] first.

use std::collections::BTreeMap;
use std::fmt;

pub mod redact;
pub use redact::{redact_text, KeyValueRule, RedactionRule, RedactionSpan, Redactor, TokenPattern};

/// Holds secret values by name. Values are never printed by `Debug`.
#[derive(Clone, Default)]
pub struct SecretStore {
    values: BTreeMap<String, String>,
}

impl fmt::Debug for SecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&String> = self.values.keys().collect();
        f.debug_struct("SecretStore")
            .field("names", &names)
            .finish()
    }
}

impl SecretStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a value under `name`, replacing any previous value.
    pub fn insert(&mut self, name: &str, value: &str) {
        self.values.insert(name.to_owned(), value.to_owned());
    }

    /// Returns the raw value for an adapter that must use it. Never log the result.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Replaces every stored value found in `text` with `[REDACTED:<name>]`.
    /// Longer values are replaced first so overlapping values cannot leak a suffix.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        let mut entries: Vec<(&String, &String)> = self.values.iter().collect();
        entries.sort_by_key(|(_, value)| std::cmp::Reverse(value.len()));
        let mut out = text.to_owned();
        for (name, value) in entries {
            if !value.is_empty() {
                out = out.replace(value.as_str(), &format!("[REDACTED:{name}]"));
            }
        }
        out
    }

    /// Redaksi lengkap: nilai tersimpan dahulu, lalu pola umum (lihat [`redact::redact_text`]).
    #[must_use]
    pub fn redact_all(&self, text: &str) -> String {
        redact::redact_text(&self.redact(text))
    }
}

#[cfg(test)]
#[path = "secrets_test.rs"]
mod secrets_test;
