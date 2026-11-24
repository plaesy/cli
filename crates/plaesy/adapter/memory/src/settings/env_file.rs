//! Secrets file `<root>/.plaesy/.env`, read next to `settings.json`.
//!
//! The format is the same `KEY=VALUE` lines Hermes uses: blank lines and `#` comments are skipped,
//! an optional `export ` prefix is ignored, and a value may be wrapped in matching quotes. An
//! environment variable with the same name always wins over the file, so existing setups keep working.
//! Values are never printed.

use crate::memory::error::MemoryError;
use crate::memory::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// File name inside the `.plaesy` folder.
pub const ENV_FILE_NAME: &str = ".env";

/// Path of the secrets file for the default root: `<root>/.plaesy/.env`.
pub fn default_env_path() -> Option<PathBuf> {
    super::default_root().map(|root| file_in_root(&root))
}

/// Secrets file location for an explicit root.
pub fn file_in_root(root: &Path) -> PathBuf {
    root.join(super::SETTINGS_DIR).join(ENV_FILE_NAME)
}

/// Read `KEY=VALUE` pairs from `path`. A missing file gives an empty map.
///
/// # Errors
///
/// - `ConfigError` if the path is a symlink or not a regular file, or cannot be read
pub fn load(path: &Path) -> Result<BTreeMap<String, String>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => {
            return Err(MemoryError::ConfigError(format!(
                "cannot inspect {}: {error}",
                path.display()
            )));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(MemoryError::ConfigError(format!(
            "secrets path is not a regular file: {}",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| MemoryError::ConfigError(format!("cannot read {}: {e}", path.display())))?;
    Ok(parse(&text))
}

/// Parse dotenv-style text. Later lines override earlier ones.
pub fn parse(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !is_valid_key(key) {
            continue;
        }
        map.insert(key.to_string(), parse_value(value.trim()));
    }
    map
}

fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn parse_value(value: &str) -> String {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return value[1..value.len() - 1].to_string();
        }
    }
    // Unquoted: a ` #` starts an inline comment.
    match value.find(" #") {
        Some(index) => value[..index].trim_end().to_string(),
        None => value.to_string(),
    }
}

/// Value of `key`: the environment variable when it is set and not empty, else the entry in
/// `<root>/.plaesy/.env` for the default root.
///
/// # Errors
///
/// - `ConfigError` when the secrets file exists but cannot be read
pub fn value(key: &str) -> Result<Option<String>> {
    if let Some(v) = std::env::var(key).ok().filter(|v| !v.is_empty()) {
        return Ok(Some(v));
    }
    let Some(path) = default_env_path() else {
        return Ok(None);
    };
    Ok(load(&path)?.remove(key).filter(|v| !v.is_empty()))
}

#[cfg(test)]
#[path = "env_file_test.rs"]
mod env_file_test;
