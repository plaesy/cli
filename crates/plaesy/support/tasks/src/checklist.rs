//! A task's `contracts.json` checklist: each item has a command that proves it
//! and an expected value its output must reach.
//!
//! An item is `pending` until it first passes, `passed` while its last run
//! passed, and `failed` while its last run failed. Every failed run is kept in
//! the item's history. A task is done when every item is `passed`.
//! Port of `scripts/internal/checklist` (Go).

use std::fs;
use std::io;
use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::expected::{self, Outcome};

/// Errors while loading or saving a checklist.
#[derive(Debug, Error)]
pub enum ChecklistError {
    #[error("{path}: {source}")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path}: item {id:?} needs a valid single-component id, run and expected")]
    Incomplete { path: String, id: String },
    #[error("writing {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: io::Error,
    },
}

/// Status of one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Pending,
    Passed,
    Failed,
}

/// One failed run of an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub at: String,
    pub actual: String,
    pub run: String,
}

/// Evidence a passed item stores. `output` is relative to the project root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub output: String,
    pub exit_code: i32,
    pub sha256: String,
}

/// One condition of the task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub check: String,
    pub run: String,
    pub expected: String,
    pub status: Status,
    #[serde(default)]
    pub histories: Vec<History>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Receipt>,
    /// What kind of evidence the check gives: `build` (compiler, test runner, grep, file test)
    /// or `doc` (a written document). Such items are exempt from the rule that every item must
    /// invoke the plaesy binary. Any other value, or none, leaves the item subject to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

/// Contents of a task's `contracts.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checklist {
    pub id: String,
    pub checklist: Vec<Item>,
}

/// Captured result of one command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
}

/// Loads and validates a `contracts.json`. Unknown fields are an error, so a
/// misspelt key cannot silently drop a check.
///
/// # Errors
///
/// Read or parse failures, or an item missing `id`, `run`, or `expected`.
pub fn load(path: &Path) -> Result<Checklist, ChecklistError> {
    let shown = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|source| ChecklistError::Read {
        path: shown.clone(),
        source,
    })?;
    parse_checklist(&text, shown)
}

pub fn load_from_handle(
    handle: &crate::memory::fs_safety::DirHandle,
    filename: &str,
) -> Result<Checklist, ChecklistError> {
    let name = checklist_filename(filename).map_err(|source| ChecklistError::Read {
        path: filename.to_owned(),
        source,
    })?;
    let shown = filename.to_owned();
    let mut file = handle
        .open_file_read_nofollow(name)
        .map_err(|source| ChecklistError::Read {
            path: shown.clone(),
            source,
        })?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).map_err(|source| ChecklistError::Read {
        path: shown.clone(),
        source,
    })?;
    parse_checklist(&text, shown)
}

fn parse_checklist(text: &str, shown: String) -> Result<Checklist, ChecklistError> {
    let checklist: Checklist =
        serde_json::from_str(text).map_err(|source| ChecklistError::Parse {
            path: shown.clone(),
            source,
        })?;
    if let Some(bad) = checklist.checklist.iter().find(|it| {
        let mut components = Path::new(&it.id).components();
        !matches!(
            (components.next(), components.next()),
            (Some(std::path::Component::Normal(_)), None)
        ) || it.id.contains(['/', '\\'])
            || it.run.is_empty()
            || it.expected.is_empty()
    }) {
        return Err(ChecklistError::Incomplete {
            path: shown,
            id: bad.id.clone(),
        });
    }
    Ok(checklist)
}

/// Writes the checklist back, replacing the file in one step.
///
/// # Errors
///
/// Serialization or filesystem failures.
pub fn save(path: &Path, checklist: &Checklist) -> Result<(), ChecklistError> {
    let shown = path.display().to_string();
    let write_err = |source| ChecklistError::Write {
        path: shown.clone(),
        source,
    };
    let mut json =
        serde_json::to_string_pretty(checklist).map_err(|source| ChecklistError::Parse {
            path: shown.clone(),
            source,
        })?;
    json.push('\n');
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, json).map_err(write_err)?;
    fs::rename(&tmp, path).map_err(write_err)
}

pub fn save_to_handle(
    handle: &crate::memory::fs_safety::DirHandle,
    filename: &str,
    checklist: &Checklist,
) -> Result<(), ChecklistError> {
    let name = checklist_filename(filename).map_err(|source| ChecklistError::Write {
        path: filename.to_owned(),
        source,
    })?;
    let shown = filename.to_owned();
    let write_err = |source| ChecklistError::Write {
        path: shown.clone(),
        source,
    };
    let mut json =
        serde_json::to_string_pretty(checklist).map_err(|source| ChecklistError::Parse {
            path: shown.clone(),
            source,
        })?;
    json.push('\n');
    use std::io::Write;

    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (tmp_name, mut file) = (0..100)
        .find_map(|_| {
            let sequence = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let candidate = format!(".{filename}.tmp-{}-{sequence}", std::process::id());
            match handle.create_new_file(Path::new(&candidate)) {
                Ok(file) => Some(Ok((candidate, file))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not allocate a unique checklist temporary file",
            ))
        })
        .map_err(write_err)?;
    let temp = Path::new(&tmp_name);
    let result = (|| -> io::Result<()> {
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        handle.replace_within(temp, name)?;
        handle.sync()
    })();
    if result.is_err() {
        let _ = handle.remove_file(temp);
    }
    result.map_err(write_err)
}

fn checklist_filename(filename: &str) -> io::Result<&Path> {
    let path = Path::new(filename);
    if path.components().count() == 1
        && matches!(
            path.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        Ok(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "checklist filename must be one normal path component",
        ))
    }
}

/// Runs every item's command and sets its status.
///
/// `run_command` executes one command line. An `Err` means the command could
/// not be started, and it counts as a failed run.
pub fn run<F>(checklist: &mut Checklist, mut run_command: F, now: DateTime<Utc>)
where
    F: FnMut(&str) -> Result<CommandOutput, String>,
{
    for item in &mut checklist.checklist {
        match run_one(item, &mut run_command) {
            None => item.status = Status::Passed,
            Some(actual) => {
                item.status = Status::Failed;
                item.histories.push(History {
                    at: now.to_rfc3339_opts(SecondsFormat::Secs, true),
                    actual,
                    run: item.run.clone(),
                });
            }
        }
    }
}

/// Returns `None` when the item passes, or the failure message.
fn run_one<F>(item: &Item, run_command: &mut F) -> Option<String>
where
    F: FnMut(&str) -> Result<CommandOutput, String>,
{
    let output = match run_command(&item.run) {
        Ok(output) => output,
        Err(message) => return Some(message),
    };
    if output.exit_code != 0 {
        return Some(format!("exit code {}", output.exit_code));
    }
    let judged = expected::judge(&item.expected, last_line(&output.stdout));
    if judged.outcome == Outcome::Met {
        return None;
    }
    if judged.reason.is_empty() {
        Some(format!(
            "result {}: actual {:?}, expected {:?} was not met. Accepted expected values: {}",
            outcome_label(judged.outcome),
            judged.actual,
            item.expected,
            expected::ACCEPTED_VALUES
        ))
    } else {
        Some(judged.reason)
    }
}

fn outcome_label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Met => "met",
        Outcome::NotMet => "not_met",
        Outcome::NotMeasurable => "not_measurable",
    }
}

/// Reports whether every item is passed. An empty checklist is not done.
#[must_use]
pub fn is_done(checklist: &Checklist) -> bool {
    !checklist.checklist.is_empty()
        && checklist
            .checklist
            .iter()
            .all(|it| it.status == Status::Passed)
}

/// Passed items and total items.
#[must_use]
pub fn progress(checklist: &Checklist) -> (usize, usize) {
    let passed = checklist
        .checklist
        .iter()
        .filter(|it| it.status == Status::Passed)
        .count();
    (passed, checklist.checklist.len())
}

fn last_line(out: &str) -> &str {
    out.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
}

#[cfg(test)]
#[path = "checklist_test.rs"]
mod checklist_test;
