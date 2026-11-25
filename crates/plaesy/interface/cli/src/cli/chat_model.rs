//! `/model` for `plaesy chat`: list the configured providers, pick one, and save the pick.
//!
//! A pick moves its provider to the front of `delegate.providers` in `.plaesy/config.toml`, where
//! `default_provider` looks first, and writes the model id into that entry. Other keys stay as they are.
//! The first save copies `.plaesy/settings.json` into `config.toml` when the TOML file does not exist yet.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value as JsonValue;

use super::chat_text::Lang;
use crate::config::project::{config_path, legacy_settings_path};
use crate::config::delegate::DelegateProviderConfig;

/// A parsed `/model` argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub provider: String,
    pub model: Option<String>,
}

impl Pick {
    /// The string `open_provider` takes: `name` or `name@model`.
    #[must_use]
    pub fn target(&self) -> String {
        match &self.model {
            Some(model) => format!("{}@{model}", self.provider),
            None => self.provider.clone(),
        }
    }
}

/// Numbered provider list. `*` marks the active provider; disabled ones are shown too.
#[must_use]
pub fn render(providers: &[DelegateProviderConfig], active: &str) -> String {
    providers
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let mark = if p.name == active { '*' } else { ' ' };
            let model = if p.model.is_empty() {
                "-"
            } else {
                p.model.as_str()
            };
            let state = if p.enabled { "" } else { "  (nonaktif)" };
            format!("{mark} {}. {}  {model}{state}", index + 1, p.name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One line that says how to pick.
#[must_use]
pub fn hint(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "pick with /model <number> or /model <provider>@<model>; the pick is saved",
        Lang::Id => "pilih dengan /model <nomor> atau /model <penyedia>@<model>; pilihan disimpan",
    }
}

/// Reads a `/model` argument: a list number, `provider`, `provider@model`, or a bare model id for
/// the active provider (the older form, kept so `/model <name>` still works).
///
/// # Errors
///
/// An unknown number, an unknown provider, or a provider that is not enabled.
pub fn parse(
    arg: &str,
    providers: &[DelegateProviderConfig],
    active: &str,
) -> Result<Pick, String> {
    let arg = arg.trim();
    if let Ok(number) = arg.parse::<usize>() {
        let entry = number
            .checked_sub(1)
            .and_then(|index| providers.get(index))
            .ok_or_else(|| format!("nomor penyedia tidak ada: {arg}"))?;
        check_enabled(entry)?;
        return Ok(Pick {
            provider: entry.name.clone(),
            model: None,
        });
    }
    let (name, model) = match arg.split_once('@') {
        Some((name, model)) => (name.trim(), Some(model.trim()).filter(|m| !m.is_empty())),
        None => (arg, None),
    };
    match providers.iter().find(|p| p.name == name) {
        Some(entry) => {
            check_enabled(entry)?;
            Ok(Pick {
                provider: name.to_owned(),
                model: model.map(str::to_owned),
            })
        }
        // Not a provider name: a bare model id for the active provider, as before.
        None if model.is_none() && !name.is_empty() => Ok(Pick {
            provider: active.to_owned(),
            model: Some(name.to_owned()),
        }),
        None => Err(format!("penyedia tidak dikenal: {name}")),
    }
}

fn check_enabled(entry: &DelegateProviderConfig) -> Result<(), String> {
    if entry.enabled {
        Ok(())
    } else {
        Err(format!("penyedia nonaktif: {}", entry.name))
    }
}

/// The settings text after `pick` is applied. The provider moves to the front of `providers` and
/// gets the model id. `providers` is the effective list, so a file with no `delegate` section gets it.
///
/// # Errors
///
/// Invalid TOML in `config`, a `delegate` that is not a table, or an unknown provider.
pub fn apply(
    config: &str,
    providers: &[DelegateProviderConfig],
    pick: &Pick,
) -> Result<String, String> {
    let mut list = providers.to_vec();
    let index = list
        .iter()
        .position(|p| p.name == pick.provider)
        .ok_or_else(|| format!("penyedia tidak dikenal: {}", pick.provider))?;
    let mut entry = list.remove(index);
    if let Some(model) = &pick.model {
        entry.model = model.clone();
    }
    list.insert(0, entry);

    let mut root: toml::Table = if config.trim().is_empty() {
        toml::Table::new()
    } else {
        config
            .parse()
            .map_err(|err| format!("config.toml tidak valid: {err}"))?
    };
    let delegate = root
        .entry("delegate".to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| "delegate harus berupa tabel".to_owned())?;
    let encoded = toml::Value::try_from(&list).map_err(|err| err.to_string())?;
    delegate.insert("providers".to_owned(), encoded);
    toml::to_string_pretty(&root).map_err(|err| err.to_string())
}

/// Converts the text of an old `settings.json` into TOML, so the first save keeps every section.
fn from_legacy_json(text: &str) -> Result<String, String> {
    let value: JsonValue = serde_json::from_str(text)
        .map_err(|err| format!("settings.json tidak valid: {err}"))?;
    let table = toml::Value::try_from(value).map_err(|err| {
        format!("settings.json tidak bisa diubah ke TOML (mungkin ada null): {err}")
    })?;
    toml::to_string_pretty(&table).map_err(|err| err.to_string())
}

/// Reads a file, returning `None` when it does not exist.
fn read_text(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

/// Saves `pick` to `<repo>/.plaesy/config.toml`, creating the folder when missing. When that file
/// does not exist yet, the old `settings.json` is the starting text.
///
/// # Errors
///
/// Read, write, or validation failures, each with the path.
pub fn save(repo: &Path, providers: &[DelegateProviderConfig], pick: &Pick) -> Result<(), String> {
    let toml_path = config_path(repo);
    let start = match read_text(&toml_path)? {
        Some(text) => text,
        None => match read_text(&legacy_settings_path(repo))? {
            Some(legacy) => from_legacy_json(&legacy)?,
            None => String::new(),
        },
    };
    let out = apply(&start, providers, pick)?;
    if let Some(parent) = toml_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    fs::write(&toml_path, format!("{out}\n"))
        .map_err(|err| format!("{}: {err}", toml_path.display()))
}

#[cfg(test)]
#[path = "chat_model_test.rs"]
mod chat_model_test;
