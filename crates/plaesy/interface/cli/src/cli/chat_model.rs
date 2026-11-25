//! `/model` for `plaesy chat`: list the configured providers, pick one, and save the pick.
//!
//! A pick moves its provider to the front of `delegate.providers` in `.plaesy/settings.json`, where
//! `default_provider` looks first, and writes the model id into that entry. Other keys stay as they are.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{json, Value};

use super::chat_text::Lang;
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
/// Invalid JSON in `settings`, a non-object root or `delegate`, or an unknown provider.
pub fn apply(
    settings: &str,
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

    let mut root: Value = if settings.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(settings).map_err(|err| format!("settings.json tidak valid: {err}"))?
    };
    let object = root
        .as_object_mut()
        .ok_or_else(|| "settings.json harus berupa objek".to_owned())?;
    let delegate = object
        .entry("delegate")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "delegate harus berupa objek".to_owned())?;
    let encoded = serde_json::to_value(&list).map_err(|err| err.to_string())?;
    delegate.insert("providers".to_owned(), encoded);
    serde_json::to_string_pretty(&root).map_err(|err| err.to_string())
}

/// Saves `pick` to the settings file at `path`, creating the file and folder when missing.
///
/// # Errors
///
/// Read, write, or validation failures, each with the path.
pub fn save(path: &Path, providers: &[DelegateProviderConfig], pick: &Pick) -> Result<(), String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(format!("{}: {err}", path.display())),
    };
    let out = apply(&text, providers, pick)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    fs::write(path, format!("{out}\n")).map_err(|err| format!("{}: {err}", path.display()))
}

#[cfg(test)]
#[path = "chat_model_test.rs"]
mod chat_model_test;
