//! Local, opt-in telemetry: counts command runs, errors and durations in
//! `.plaesy/telemetry.jsonl`. Nothing is sent over the network. Recording is off unless
//! `.plaesy/config.json` sets `telemetry.enabled` to `true`.

use crate::config::{manager::ConfigManager, schema::UnifiedConfig, ConfigError};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// File name of the event log, inside `.plaesy/`.
pub const LOG_FILE: &str = "telemetry.jsonl";

/// One recorded command run. It holds no arguments, paths or messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Command group name, such as `tasks` or `memory`.
    pub command: String,
    /// Whether the command returned success.
    pub ok: bool,
    /// Wall-clock duration of the command, in milliseconds.
    pub duration_ms: u64,
    /// Time the event was recorded, as Unix milliseconds.
    pub at_ms: u64,
}

/// Errors raised by telemetry.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Totals for one command name.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommandStats {
    pub runs: u64,
    pub errors: u64,
    pub total_ms: u64,
}

/// Aggregated view of the event log.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Summary {
    /// Per-command totals, sorted by command name.
    pub commands: BTreeMap<String, CommandStats>,
    /// Lines that could not be parsed as events. They are skipped, not fatal.
    pub malformed_lines: u64,
}

impl Summary {
    /// Total runs across all commands.
    #[must_use]
    pub fn total_runs(&self) -> u64 {
        self.commands.values().map(|s| s.runs).sum()
    }

    /// Total failed runs across all commands.
    #[must_use]
    pub fn total_errors(&self) -> u64 {
        self.commands.values().map(|s| s.errors).sum()
    }
}

/// `<root>/.plaesy/config.json`, the unified configuration that holds the telemetry switch.
#[must_use]
pub fn config_path(root: &Path) -> PathBuf {
    root.join(".plaesy").join("config.json")
}

/// `<root>/.plaesy/telemetry.jsonl`, the event log.
#[must_use]
pub fn log_path(root: &Path) -> PathBuf {
    root.join(".plaesy").join(LOG_FILE)
}

/// Whether telemetry is enabled for the project at `root`. A missing config file means disabled.
///
/// # Errors
///
/// The config file exists but cannot be read or parsed.
pub fn is_enabled(root: &Path) -> Result<bool, Error> {
    let manager = ConfigManager::new(config_path(root), UnifiedConfig::default());
    let cfg = manager.load()?;
    Ok(cfg.telemetry.as_ref().is_some_and(|t| t.enabled))
}

/// Appends one command run to the log when telemetry is enabled. When it is disabled, nothing
/// is written and the log is not created.
///
/// # Errors
///
/// The config cannot be read, or the log cannot be written.
pub fn record(root: &Path, command: &str, ok: bool, duration: Duration) -> Result<(), Error> {
    if !is_enabled(root)? {
        return Ok(());
    }
    let event = Event {
        command: command.to_owned(),
        ok,
        duration_ms: u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        at_ms: now_ms(),
    };
    append(&log_path(root), &event)
}

/// Reads the event log and totals it per command. A missing log yields an empty summary.
/// Lines that do not parse are counted in `malformed_lines` and skipped.
///
/// # Errors
///
/// The log exists but cannot be read.
pub fn summarize(root: &Path) -> Result<Summary, Error> {
    let text = match fs::read_to_string(log_path(root)) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Summary::default()),
        Err(e) => return Err(e.into()),
    };
    let mut summary = Summary::default();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        match serde_json::from_str::<Event>(line) {
            Ok(event) => {
                let stats = summary.commands.entry(event.command).or_default();
                stats.runs += 1;
                if !event.ok {
                    stats.errors += 1;
                }
                stats.total_ms = stats.total_ms.saturating_add(event.duration_ms);
            }
            Err(_) => summary.malformed_lines += 1,
        }
    }
    Ok(summary)
}

fn append(path: &Path, event: &Event) -> Result<(), Error> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_vec(event)?;
    line.push(b'\n');
    // One write of one short line on an append-mode handle keeps concurrent runs from
    // interleaving records.
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(&line)?;
    Ok(())
}

fn now_ms() -> u64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(since_epoch.as_millis()).unwrap_or(0)
}

#[cfg(test)]
#[path = "telemetry_test.rs"]
mod telemetry_test;
