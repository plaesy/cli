//! The run state under `autonomous_loop` in `.plaesy/state.json`. Other keys in
//! that file are kept as they are.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

const STATE_KEY: &str = "autonomous_loop";

/// Errors reading or writing the state file.
#[derive(Debug, Error)]
pub enum StateError {
    #[error("{}: {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{}: {message}", path.display())]
    Parse {
        path: std::path::PathBuf,
        message: String,
    },
}

/// The run state the driver owns.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub status: String,
    pub current_iteration: u32,
    pub consecutive_failures: u32,
    pub max_consecutive_failures: u32,
    pub clean_cycles: u32,
    /// Process id of the driver that last wrote the heartbeat.
    pub driver_pid: i64,
    /// RFC 3339 UTC time of the last heartbeat.
    pub heartbeat_at: String,
    /// RFC 3339 UTC time the run first started. Set once and kept across restarts.
    pub started_at: String,
    /// Sum of the token counts steps reported.
    pub tokens_used: u64,
    /// True once any step reported a count. Until then, usage is UNKNOWN.
    pub tokens_reported: bool,
}

/// Reads the state. A missing file or missing section is an empty state.
///
/// # Errors
///
/// [`StateError`] for a read failure or a file that is not a JSON object.
pub fn load(path: &Path) -> Result<State, StateError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(State::default()),
        Err(source) => {
            return Err(StateError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    if text.trim().is_empty() {
        return Ok(State::default());
    }
    let doc: Map<String, Value> = serde_json::from_str(&text).map_err(|e| StateError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let Some(section) = doc.get(STATE_KEY) else {
        return Ok(State::default());
    };
    serde_json::from_value(section.clone()).map_err(|e| StateError::Parse {
        path: path.to_path_buf(),
        message: format!("{STATE_KEY}: {e}"),
    })
}

/// Writes the state, keeping every other key, through a temp file and a rename.
///
/// # Errors
///
/// [`StateError`] for read, parse, or write failures.
pub fn save(path: &Path, state: &State) -> Result<(), StateError> {
    let io_err = |source| StateError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut doc: Map<String, Value> = match fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => {
            serde_json::from_str(&text).map_err(|e| StateError::Parse {
                path: path.to_path_buf(),
                message: e.to_string(),
            })?
        }
        Ok(_) => Map::new(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(io_err(e)),
    };
    doc.insert(
        STATE_KEY.to_owned(),
        serde_json::to_value(state).map_err(|e| StateError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?,
    );
    let mut text = serde_json::to_string_pretty(&doc).map_err(|e| StateError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    text.push('\n');
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    fs::write(&tmp, text).map_err(io_err)?;
    fs::rename(&tmp, path).map_err(io_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_keeps_other_keys_and_round_trips_the_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        fs::write(
            &path,
            r#"{"project":"demo","autonomous_loop":{"status":"running"}}"#,
        )
        .unwrap();
        let state = State {
            status: "paused".into(),
            current_iteration: 4,
            ..State::default()
        };
        save(&path, &state).unwrap();
        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["project"], "demo");
        assert_eq!(load(&path).unwrap(), state);
    }

    #[test]
    fn missing_file_is_an_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load(&dir.path().join("nope.json")).unwrap(),
            State::default()
        );
    }
}
