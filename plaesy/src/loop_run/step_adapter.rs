//! The adapter that performs one loop step.
//!
//! `command` and `openai` steps use the same provider types as `plaesy delegate`.
//! With no `step` object, the step is the `step_command` shell job, and a missing
//! command fails every step, so the run stops with a report instead of passing.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::config::Config;
use super::jobs;
use super::queue::Finding;
use crate::config::delegate::DelegateProviderConfig;
use crate::delegate::{self, providers::Getenv, Request};

/// The `step` object of `.plaesy/loop.json`. The key itself is never in the file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StepConfig {
    /// `command` or `openai`.
    pub kind: String,
    pub command: String,
    pub args: Vec<String>,
    pub timeout_seconds: i64,
    pub base_url: String,
    pub model: String,
    /// Environment variable that holds the key.
    pub api_key_env: String,
}

/// What one iteration reports back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StepResult {
    /// True when the iteration finished without a failure.
    pub clean: bool,
    /// Facts the step could not establish. The run continues with them as UNKNOWN.
    pub unknown: Vec<String>,
    /// Problems to queue.
    pub findings: Vec<Finding>,
    /// Tokens reported for this iteration. `None` means UNKNOWN.
    pub tokens: Option<u64>,
}

/// One iteration. The driver decides Stop or Completion from thresholds, not from the step's claim.
pub type Step = Box<dyn FnMut(u32) -> Result<StepResult, String>>;

/// Name a step's provider carries in its errors.
const STEP_PROVIDER: &str = "loop-step";

/// Builds the step that `cfg` selects.
///
/// # Errors
///
/// A configuration problem, such as a missing command or endpoint.
pub fn new_step(
    cfg: &Config,
    work_dir: &Path,
    jobs_dir: &Path,
    getenv: Getenv,
) -> Result<Step, String> {
    let Some(step) = cfg.step.clone() else {
        return Ok(step_command_job(
            cfg.step_command.clone(),
            work_dir.to_path_buf(),
            jobs_dir.to_path_buf(),
        ));
    };
    if step.timeout_seconds < 0 {
        return Err("loop step: timeout_seconds must not be negative".into());
    }
    let entry = match step.kind.as_str() {
        "command" => {
            if step.command.trim().is_empty() {
                return Err(r#"loop step: kind "command" needs a command"#.into());
            }
            DelegateProviderConfig {
                name: STEP_PROVIDER.into(),
                kind: "cli".into(),
                enabled: true,
                command: step.command.clone(),
                args: step.args.clone(),
                ..DelegateProviderConfig::default()
            }
        }
        "openai" => {
            if step.base_url.trim().is_empty() || step.model.trim().is_empty() {
                return Err(r#"loop step: kind "openai" needs base_url and model"#.into());
            }
            DelegateProviderConfig {
                name: STEP_PROVIDER.into(),
                kind: "openai".into(),
                enabled: true,
                endpoint: step.base_url.clone(),
                model: step.model.clone(),
                api_key_env: step.api_key_env.clone(),
                ..DelegateProviderConfig::default()
            }
        }
        other => {
            return Err(format!(
                "loop step: unknown kind {other:?} (supported: command, openai)"
            ))
        }
    };
    let provider =
        delegate::providers::build(&entry, getenv).map_err(|e| format!("loop step: {e}"))?;
    let timeout = Duration::from_secs(step.timeout_seconds as u64);
    let work_dir = work_dir.to_path_buf();
    let jobs_dir = jobs_dir.to_path_buf();

    Ok(Box::new(move |iteration: u32| {
        let req = Request {
            prompt: step_prompt(iteration, &work_dir),
            dir: work_dir.display().to_string(),
            timeout,
            ..Request::default()
        };
        let resp = delegate::run(provider.as_ref(), &req).map_err(|e| e.message)?;
        std::fs::create_dir_all(&jobs_dir).map_err(|e| e.to_string())?;
        let out = jobs_dir.join(format!("step-{iteration}.out"));
        std::fs::write(&out, format!("{}\n", resp.text)).map_err(|e| e.to_string())?;
        let tokens = match (resp.input_tokens, resp.output_tokens) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        };
        Ok(StepResult {
            clean: true,
            unknown: Vec::new(),
            findings: Vec::new(),
            tokens,
        })
    }))
}

fn step_prompt(iteration: u32, work_dir: &Path) -> String {
    format!(
        "Loop iteration {iteration} for the project at {}. Perform one step of the autonomous loop and report what you changed and whether the step passed.",
        work_dir.display()
    )
}

/// Runs `step_command` as a background job and reports whether it exited 0.
fn step_command_job(step_command: String, work_dir: PathBuf, jobs_dir: PathBuf) -> Step {
    Box::new(move |iteration: u32| {
        if step_command.trim().is_empty() {
            return Err("no step_command in .plaesy/loop.json".into());
        }
        let name = format!("step-{iteration}");
        let argv = vec!["bash".to_owned(), "-c".to_owned(), step_command.clone()];
        let job =
            jobs::start(Some(&work_dir), &jobs_dir, &name, &argv).map_err(|e| e.to_string())?;
        job.wait();
        let result = jobs::read_result(&jobs_dir, &name).map_err(|e| e.to_string())?;
        if !result.finished {
            return Err("step job did not write its exit file".into());
        }
        Ok(StepResult {
            clean: result.exit_code == 0,
            ..StepResult::default()
        })
    })
}

/// Arc-wrapped environment lookup that reads the process environment.
#[must_use]
pub fn process_env() -> Getenv {
    Arc::new(|name: &str| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_openai_step_without_an_endpoint_is_refused() {
        let cfg = Config {
            step: Some(StepConfig {
                kind: "openai".into(),
                ..StepConfig::default()
            }),
            ..Config::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let err = new_step(&cfg, dir.path(), dir.path(), process_env())
            .err()
            .expect("refused");
        assert!(err.contains("base_url and model"));
    }

    #[test]
    fn no_step_command_fails_the_step_instead_of_passing_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut step = new_step(&Config::default(), dir.path(), dir.path(), process_env()).unwrap();
        assert!(step(1).is_err());
    }
}
