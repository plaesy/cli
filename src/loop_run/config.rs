//! `.plaesy/loop.json`: the decisions a run would otherwise ask the person for.
//! Every field has a default, so a run with no file never prompts.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::step_adapter::StepConfig;

/// Errors reading the loop config.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{}: {source}", path.display())]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{}: {source}", path.display())]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// One numeric bar a measurement must reach.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Threshold {
    pub name: String,
    pub min: f64,
    pub mandatory: bool,
}

/// Loop settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Clean cycles in a row before Completion is checked. Zero disables the check.
    pub clean_cycles_for_completion: u32,
    /// Iteration cap. Zero means none.
    pub max_iterations: u32,
    /// Stop after this many failed cycles in a row.
    pub max_consecutive_failures: u32,
    /// Hard iteration cap. Zero turns it off.
    pub max_run_iterations: u32,
    /// Wall-clock cap in minutes, measured from `started_at`. Zero turns it off.
    pub max_wall_minutes: u64,
    /// Token cap. Applies only once a step reports a count. Zero turns it off.
    pub max_tokens: u64,
    /// Per-step timeout for the legacy shell adapter. Zero disables it.
    pub legacy_step_timeout_seconds: u64,
    /// Measurement command per named measurement.
    pub measure: BTreeMap<String, String>,
    /// Shell command that performs one step, used when no `step` object is set.
    pub step_command: String,
    /// The adapter that performs one step.
    pub step: Option<StepConfig>,
    /// Constitution thresholds Completion is checked against.
    pub thresholds: Vec<Threshold>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            clean_cycles_for_completion: 3,
            max_iterations: 0,
            max_consecutive_failures: 2,
            // ASSUMED defaults, chosen by the assistant and not measured.
            max_run_iterations: 500,
            max_wall_minutes: 240,
            max_tokens: 5_000_000,
            legacy_step_timeout_seconds: 900,
            measure: BTreeMap::new(),
            step_command: String::new(),
            step: None,
            thresholds: vec![Threshold {
                name: "coverage".into(),
                min: 90.0,
                mandatory: true,
            }],
        }
    }
}

/// Reads `.plaesy/loop.json` over the defaults. A missing file is the defaults. An
/// unknown key is an error, so a misspelt key cannot silently drop a setting.
///
/// # Errors
///
/// [`ConfigError`] for a read failure or invalid JSON.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    serde_json::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_caps() {
        let cfg = Config::default();
        assert_eq!(cfg.max_run_iterations, 500);
        assert_eq!(cfg.max_wall_minutes, 240);
        assert_eq!(cfg.thresholds[0].min, 90.0);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let typo: Result<Config, _> = serde_json::from_str(r#"{"max_run_iteration": 3}"#);
        assert!(typo.is_err());
    }

    #[test]
    fn missing_file_is_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load(&dir.path().join("loop.json")).unwrap(),
            Config::default()
        );
    }
}
