//! The project configuration file, read by every crate that needs a project setting.
//!
//! `.plaesy/config.toml` is the file. `.plaesy/settings.json` is read only while `config.toml` does
//! not exist, so a repository that has not migrated keeps working. Both are read as JSON values, so
//! callers keep the same lookups whichever file they came from.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// `<repo>/.plaesy/config.toml`.
#[must_use]
pub fn config_path(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("config.toml")
}

/// `<repo>/.plaesy/settings.json`, the file that TOML replaces.
#[must_use]
pub fn legacy_settings_path(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("settings.json")
}

/// Reads a file, returning `None` when it does not exist.
fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("reading {}: {err}", path.display())),
    }
}

/// The project configuration as a JSON value: `<repo>/.plaesy/config.toml`, or the older
/// `settings.json` while that is absent. Neither file gives `Value::Null`.
///
/// # Errors
///
/// A file that cannot be read, or that does not parse as its format.
pub fn read_project_value(repo: &Path) -> Result<Value, String> {
    read_config_dir(&repo.join(".plaesy"))
}

/// The user configuration in the home folder: `~/.plaesy/config.toml`, or `settings.json` while that
/// is absent. `None` when there is no home folder.
///
/// # Errors
///
/// A file that cannot be read, or that does not parse as its format.
pub fn read_user_value() -> Result<Value, String> {
    match dirs::home_dir() {
        Some(home) => read_config_dir(&home.join(".plaesy")),
        None => Ok(Value::Null),
    }
}

/// Reads `config.toml` in `folder`, or `settings.json` there while it is absent. `folder` is the
/// `.plaesy` folder itself.
///
/// # Errors
///
/// A file that cannot be read, or that does not parse as its format.
pub fn read_config_dir(folder: &Path) -> Result<Value, String> {
    let toml_path = folder.join("config.toml");
    if let Some(text) = read_optional(&toml_path)? {
        let table: toml::Table = text
            .parse()
            .map_err(|err| format!("parsing {}: {err}", toml_path.display()))?;
        return serde_json::to_value(table).map_err(|err| err.to_string());
    }
    let json_path = folder.join("settings.json");
    match read_optional(&json_path)? {
        Some(text) => serde_json::from_str(&text)
            .map_err(|err| format!("parsing {}: {err}", json_path.display())),
        None => Ok(Value::Null),
    }
}

/// [`read_project_value`] with a failure read as `Value::Null`, for lookups that treat missing
/// and broken settings the same way.
#[must_use]
pub fn read_project_value_or_null(repo: &Path) -> Value {
    read_project_value(repo).unwrap_or(Value::Null)
}

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;
