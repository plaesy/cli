//! Scheduled agent prompts under `.plaesy/automations/`.
//!
//! An automation is a prompt with an agent, a role, and one schedule: a five-field cron
//! expression or an interval such as `30m`. There is no daemon. `plaesy automation run-due`
//! evaluates every enabled automation, and an OS timer or the user may call it again.
//!
//! Each due automation records its run before anything else happens, so a second `run-due`
//! started meanwhile sees it as not due. A lock file serializes whole `run-due` passes.
//! Schedules are evaluated in UTC; the schedule engine has no IANA time zone support.
//!
//! This slice only queues runs. Starting the agent for a queued run belongs to the harness
//! slice and is not done here.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration as StdDuration, SystemTime};

use chrono::{DateTime, Utc};
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use super::CliResult;
use crate::memory::schedule::cron::{cron_is_due, CronSpec};
use crate::memory::schedule::{is_due, parse_every};

/// Longest prompt, in UTF-8 bytes.
pub const PROMPT_MAX_BYTES: usize = 4096;
/// Longest automation name, in characters.
const NAME_MAX_CHARS: usize = 64;
/// A `run-due` lock older than this is treated as left behind by a dead process.
const LOCK_MAX_AGE: StdDuration = StdDuration::from_secs(24 * 60 * 60);
const LOCK_FILE: &str = ".run-due.lock";
const RUNS_SUFFIX: &str = ".runs.jsonl";

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Action,
}

#[derive(Debug, Subcommand)]
pub enum Action {
    /// Create an automation. Give exactly one of --cron or --every.
    Create {
        /// Unique name: letters, digits, `-` and `_`
        name: String,
        /// Prompt sent to the agent (at most 4096 bytes)
        #[arg(long)]
        prompt: String,
        /// Agent that runs the prompt
        #[arg(long)]
        agent: String,
        /// Role the agent acts as
        #[arg(long)]
        role: String,
        /// Five-field cron expression, evaluated in UTC
        #[arg(long, conflicts_with = "every")]
        cron: Option<String>,
        /// Interval such as 30m, 6h or 1d
        #[arg(long)]
        every: Option<String>,
        /// Project root containing `.plaesy/`
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// List automations with their schedule and state
    Ls {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Stop an automation from being queued until resumed
    Pause {
        name: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Allow a paused automation to be queued again
    Resume {
        name: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Delete an automation and its run history
    Rm {
        name: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Show the queued runs of one automation
    Runs {
        name: String,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Queue every enabled automation that is due now
    RunDue {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

/// The stored definition, one JSON file per automation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Automation {
    pub name: String,
    pub prompt: String,
    pub agent: String,
    pub role: String,
    /// Set when the schedule is a cron expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    /// Set when the schedule is an interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    /// Time of the most recent queued run. `None` means it never ran.
    #[serde(default)]
    pub last_run: Option<DateTime<Utc>>,
}

/// One queued run, appended to the automation's run history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub at: DateTime<Utc>,
    pub agent: String,
    pub role: String,
    pub status: String,
}

/// `<project>/.plaesy/automations`.
pub fn store_dir(project: &Path) -> PathBuf {
    project.join(".plaesy").join("automations")
}

/// Check an automation name. It becomes a file name, so path characters are refused.
///
/// # Errors
///
/// - a message when the name is empty, too long, or has characters outside the allowed set
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.chars().count() > NAME_MAX_CHARS {
        return Err(format!(
            "automation name must be 1 to {NAME_MAX_CHARS} characters"
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "automation name '{name}' may use only letters, digits, '-' and '_'"
        ));
    }
    Ok(())
}

/// Check a prompt: not blank, and at most [`PROMPT_MAX_BYTES`] bytes.
///
/// # Errors
///
/// - a message when the prompt is blank or longer than the limit
pub fn validate_prompt(prompt: &str) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("automation prompt must not be empty".into());
    }
    if prompt.len() > PROMPT_MAX_BYTES {
        return Err(format!(
            "automation prompt is {} bytes; the limit is {PROMPT_MAX_BYTES}",
            prompt.len()
        ));
    }
    Ok(())
}

/// Check that exactly one schedule is given and that it parses.
///
/// # Errors
///
/// - a message when both or neither of `cron` and `every` are set, or when the value is invalid
pub fn validate_schedule(cron: Option<&str>, every: Option<&str>) -> Result<(), String> {
    match (cron, every) {
        (Some(_), Some(_)) | (None, None) => {
            Err("give exactly one schedule: --cron or --every".into())
        }
        (Some(expr), None) => CronSpec::parse(expr).map(|_| ()).map_err(|e| e.to_string()),
        (None, Some(interval)) => parse_every(interval).map(|_| ()).map_err(|e| e.to_string()),
    }
}

fn file_for(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

fn runs_for(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}{RUNS_SUFFIX}"))
}

fn io_err(path: &Path, e: io::Error) -> String {
    format!("{}: {e}", path.display())
}

fn save(dir: &Path, automation: &Automation) -> Result<(), String> {
    let path = file_for(dir, &automation.name);
    let text = serde_json::to_string_pretty(automation).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| io_err(&path, e))
}

/// Load one automation by name.
///
/// # Errors
///
/// - a message when the name is invalid, the automation does not exist, or its file is unreadable
pub fn load(dir: &Path, name: &str) -> Result<Automation, String> {
    validate_name(name)?;
    let path = file_for(dir, name);
    let text = fs::read_to_string(&path).map_err(|e| io_err(&path, e))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The fields a new automation is created from.
#[derive(Clone, Copy, Debug)]
pub struct NewAutomation<'a> {
    pub name: &'a str,
    pub prompt: &'a str,
    pub agent: &'a str,
    pub role: &'a str,
    pub cron: Option<&'a str>,
    pub every: Option<&'a str>,
}

/// Create a new automation. It is enabled and has never run.
///
/// # Errors
///
/// - a message for an invalid name, prompt, or schedule, or when the name is already taken
pub fn create(dir: &Path, spec: NewAutomation, now: DateTime<Utc>) -> Result<Automation, String> {
    let NewAutomation {
        name,
        prompt,
        agent,
        role,
        cron,
        every,
    } = spec;
    validate_name(name)?;
    validate_prompt(prompt)?;
    validate_schedule(cron, every)?;
    if agent.trim().is_empty() || role.trim().is_empty() {
        return Err("agent and role must not be empty".into());
    }
    fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    if file_for(dir, name).exists() {
        return Err(format!("automation '{name}' already exists"));
    }
    let automation = Automation {
        name: name.to_owned(),
        prompt: prompt.to_owned(),
        agent: agent.to_owned(),
        role: role.to_owned(),
        cron: cron.map(str::to_owned),
        every: every.map(str::to_owned),
        enabled: true,
        created_at: now,
        last_run: None,
    };
    save(dir, &automation)?;
    Ok(automation)
}

/// Every automation in the store, sorted by name. A missing store is an empty list.
///
/// # Errors
///
/// - a message when a definition file cannot be read or parsed
pub fn list(dir: &Path) -> Result<Vec<Automation>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(dir, e)),
    };
    let mut out = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| io_err(dir, e))?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(stem) => stem.to_owned(),
            None => continue,
        };
        out.push(load(dir, &stem)?);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Pause or resume an automation.
///
/// # Errors
///
/// - a message when the automation cannot be loaded or saved
pub fn set_enabled(dir: &Path, name: &str, enabled: bool) -> Result<Automation, String> {
    let mut automation = load(dir, name)?;
    automation.enabled = enabled;
    save(dir, &automation)?;
    Ok(automation)
}

/// Delete an automation together with its run history.
///
/// # Errors
///
/// - a message when the definition does not exist or a file cannot be removed
pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    load(dir, name)?;
    let path = file_for(dir, name);
    fs::remove_file(&path).map_err(|e| io_err(&path, e))?;
    let runs = runs_for(dir, name);
    match fs::remove_file(&runs) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err(&runs, e)),
    }
}

/// Run history of one automation, oldest first.
///
/// # Errors
///
/// - a message when the automation does not exist or its history cannot be parsed
pub fn history(dir: &Path, name: &str) -> Result<Vec<Run>, String> {
    load(dir, name)?;
    let path = runs_for(dir, name);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(&path, e)),
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|e| format!("{}: {e}", path.display())))
        .collect()
}

fn is_job_due(automation: &Automation, now: DateTime<Utc>) -> Result<bool, String> {
    if let Some(expr) = &automation.cron {
        let spec = CronSpec::parse(expr).map_err(|e| e.to_string())?;
        return Ok(cron_is_due(&spec, automation.last_run, now));
    }
    if let Some(interval) = &automation.every {
        let every = parse_every(interval).map_err(|e| e.to_string())?;
        return Ok(is_due(automation.last_run, every, now));
    }
    Ok(false)
}

/// Lock held for one `run-due` pass. It is removed on drop.
struct PassLock {
    path: PathBuf,
}

impl PassLock {
    /// `Ok(None)` when another live pass holds the lock.
    fn acquire(dir: &Path) -> Result<Option<Self>, String> {
        let path = dir.join(LOCK_FILE);
        for _ in 0..2 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Some(Self { path })),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .is_some_and(|age| age > LOCK_MAX_AGE);
                    if !stale {
                        return Ok(None);
                    }
                    let _ = fs::remove_file(&path);
                }
                Err(e) => return Err(io_err(&path, e)),
            }
        }
        Ok(None)
    }
}

impl Drop for PassLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Queue every enabled automation that is due at `now`, in name order.
///
/// Returns `Ok(None)` when another pass is running, so nothing is queued twice.
///
/// # Errors
///
/// - a message when the store, a definition, or a schedule cannot be read or written
pub fn run_due(dir: &Path, now: DateTime<Utc>) -> Result<Option<Vec<(String, Run)>>, String> {
    if !dir.is_dir() {
        return Ok(Some(Vec::new()));
    }
    let Some(_lock) = PassLock::acquire(dir)? else {
        return Ok(None);
    };
    let mut queued = Vec::new();
    for mut automation in list(dir)? {
        if !automation.enabled || !is_job_due(&automation, now)? {
            continue;
        }
        // Claim before anything else, so a later pass does not queue the same run again.
        automation.last_run = Some(now);
        save(dir, &automation)?;
        let run = Run {
            at: now,
            agent: automation.agent.clone(),
            role: automation.role.clone(),
            status: "queued".into(),
        };
        append_run(dir, &automation.name, &run)?;
        queued.push((automation.name.clone(), run));
    }
    Ok(Some(queued))
}

fn append_run(dir: &Path, name: &str, run: &Run) -> Result<(), String> {
    use std::io::Write;
    let path = runs_for(dir, name);
    let line = serde_json::to_string(run).map_err(|e| e.to_string())?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| io_err(&path, e))?;
    writeln!(file, "{line}").map_err(|e| io_err(&path, e))
}

fn schedule_text(automation: &Automation) -> String {
    match (&automation.cron, &automation.every) {
        (Some(expr), _) => format!("cron {expr} (UTC)"),
        (None, Some(interval)) => format!("every {interval}"),
        (None, None) => "no schedule".into(),
    }
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Action::Create {
            name,
            prompt,
            agent,
            role,
            cron,
            every,
            path,
        } => {
            let dir = store_dir(path);
            let spec = NewAutomation {
                name,
                prompt,
                agent,
                role,
                cron: cron.as_deref(),
                every: every.as_deref(),
            };
            let created = create(&dir, spec, Utc::now())?;
            println!("created {} ({})", created.name, schedule_text(&created));
        }
        Action::Ls { path } => {
            for a in list(&store_dir(path))? {
                let state = if a.enabled { "enabled" } else { "paused" };
                println!("{}\t{}\t{}\t{}", a.name, state, schedule_text(&a), a.agent);
            }
        }
        Action::Pause { name, path } => {
            set_enabled(&store_dir(path), name, false)?;
            println!("paused {name}");
        }
        Action::Resume { name, path } => {
            set_enabled(&store_dir(path), name, true)?;
            println!("resumed {name}");
        }
        Action::Rm { name, path } => {
            remove(&store_dir(path), name)?;
            println!("removed {name}");
        }
        Action::Runs { name, path } => {
            for r in history(&store_dir(path), name)? {
                println!(
                    "{}\t{}\t{}\t{}",
                    r.at.to_rfc3339(),
                    r.status,
                    r.agent,
                    r.role
                );
            }
        }
        Action::RunDue { path } => match run_due(&store_dir(path), Utc::now())? {
            None => println!("run-due skipped: another pass is active"),
            Some(queued) if queued.is_empty() => println!("no automation is due"),
            Some(queued) => {
                for (name, run) in queued {
                    println!("queued {} at {}", name, run.at.to_rfc3339());
                }
            }
        },
    }
    Ok(())
}

#[cfg(test)]
#[path = "automation_test.rs"]
mod automation_test;
