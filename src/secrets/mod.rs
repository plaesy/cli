//! Secret references and redaction (spec R9, VS-15).
//!
//! Tasks refer to secrets by name. Raw values stay in the store. Anything written
//! to events, logs, or exports passes through [`SecretStore::redact`] first.

use std::collections::BTreeMap;
use std::fmt;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_secret_redacts_values_from_logs() {
        let mut store = SecretStore::new();
        store.insert("api_key", "sk-synthetic-123456");
        let log_line = "call provider with sk-synthetic-123456 and retry";
        let redacted = store.redact(log_line);
        assert!(!redacted.contains("sk-synthetic-123456"));
        assert!(redacted.contains("[REDACTED:api_key]"));
    }

    #[test]
    fn approval_secret_debug_output_hides_values() {
        let mut store = SecretStore::new();
        store.insert("api_key", "sk-synthetic-123456");
        let debug = format!("{store:?}");
        assert!(debug.contains("api_key"));
        assert!(!debug.contains("sk-synthetic-123456"));
    }

    #[test]
    fn approval_secret_overlapping_values_do_not_leak_suffix() {
        let mut store = SecretStore::new();
        store.insert("short", "abc");
        store.insert("long", "abcdef");
        let redacted = store.redact("token=abcdef");
        assert!(!redacted.contains("def"));
    }
}
