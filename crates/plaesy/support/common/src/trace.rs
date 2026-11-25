//! Opt-in local trace of chat turns, one JSON object per line.
//!
//! Tracing is off unless the `PLAESY_TRACE` environment variable is set. `1` (or `true`, `on`)
//! writes `.plaesy/trace.jsonl` under the current directory; any other non-empty value is the
//! file to write. Message text is never recorded, only sizes, durations, provider names, and
//! the error text a provider returned. Nothing goes to stdout or stderr, so the chat screen is
//! not disturbed.

use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Environment variable that turns tracing on and chooses the file.
pub const ENV: &str = "PLAESY_TRACE";

/// Default file, relative to the current directory, when `PLAESY_TRACE` is `1`.
pub const DEFAULT_FILE: &str = ".plaesy/trace.jsonl";

/// The trace file named by `value`, or `None` when tracing is off.
#[must_use]
pub fn target_for(value: Option<&str>) -> Option<PathBuf> {
    let value = value?.trim();
    match value {
        "" | "0" | "false" | "off" => None,
        "1" | "true" | "on" => Some(PathBuf::from(DEFAULT_FILE)),
        other => Some(PathBuf::from(other)),
    }
}

/// The trace file for this process, or `None` when tracing is off.
#[must_use]
pub fn target() -> Option<PathBuf> {
    target_for(std::env::var(ENV).ok().as_deref())
}

/// Appends one event to the trace file, if tracing is on. `fields` must be a JSON object; its
/// keys are added beside `at_ms` (Unix milliseconds) and `event`.
///
/// Tracing is best effort: a failed write is ignored, so a trace problem never breaks a turn.
pub fn event(name: &str, fields: Value) {
    if let Some(path) = target() {
        append(&path, name, fields);
    }
}

/// Appends one event to `path`. Errors are ignored, as in [`event`].
pub fn append(path: &Path, name: &str, fields: Value) {
    let mut record = json!({ "at_ms": now_ms(), "event": name });
    if let (Some(map), Value::Object(extra)) = (record.as_object_mut(), fields) {
        map.extend(extra);
    }
    let Ok(line) = serde_json::to_string(&record) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
#[path = "trace_test.rs"]
mod trace_test;
