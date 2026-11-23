//! Scheduled memory jobs (`memory schedule once|run`).

//!
//! A job runs one existing engine method (`index`, `hygiene`, `missing` or `curate`) on the vault
//! at a fixed interval. Last-run times live in the `schedule_runs` table of the same database, so
//! no second store exists. Each run appends one line to `<root>\Plaesy\schedule.log`. A failing
//! job is recorded and the next job still runs.
//!
//! Overlap: a job's `last_run` is written before it starts. A second `once` that begins meanwhile
//! sees the job as not due, so Task Scheduler firing during a long index does not run it twice.
//! If the process dies mid-run the job waits one full interval, then runs again.
//!
//! Cron expressions (`cron::CronSpec`) are parsed and evaluated in UTC by the pure `cron`
//! submodule. They are not yet a job trigger: `JobSpec` still takes only an interval.

pub mod cron;
pub mod registration;

use crate::memory::config::ScheduleConfig;
use crate::memory::error::{MemoryError, Result};
use crate::memory::hygiene::HygieneOptions;
use crate::memory::notes::WriteLock;
use crate::memory::search::SearchEngine;
use crate::memory::settings;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, OptionalExtension};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Longest sleep in `run` mode, so edits to the config are picked up within a minute.
const MAX_SLEEP: std::time::Duration = std::time::Duration::from_secs(60);

const STATE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS schedule_runs (
    job      TEXT PRIMARY KEY,
    last_run TEXT NOT NULL,
    outcome  TEXT NOT NULL
);";

/// What a job does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Sync the vault into the index.
    Index,
    /// Audit the memory (`memory hygiene`).
    Hygiene,
    /// Report what the memory is missing (`memory missing`).
    Missing,
    /// Prepare a read-only maintenance review (`memory curate`).
    Curate,
}

impl Action {
    /// Parse an action name from the config.
    ///
    /// # Errors
    ///
    /// - `ConfigError` for any name other than `index`, `hygiene`, `missing` or `curate`
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "index" => Ok(Action::Index),
            "hygiene" => Ok(Action::Hygiene),
            "missing" => Ok(Action::Missing),
            "curate" => Ok(Action::Curate),
            other => Err(MemoryError::ConfigError(format!(
                "unknown schedule action '{other}' (use index, hygiene, missing or curate)"
            ))),
        }
    }
}

/// A validated job.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    /// Unique name.
    pub name: String,
    /// Interval between runs.
    pub every: Duration,
    /// What the job does.
    pub action: Action,
}

/// Parse an interval: a positive whole number followed by `m` (minutes), `h` (hours) or `d` (days).
///
/// # Errors
///
/// - `ConfigError` for anything else, including zero and negative numbers
pub fn parse_every(s: &str) -> Result<Duration> {
    let bad =
        || MemoryError::ConfigError(format!("interval must look like 30m, 6h or 1d (got '{s}')"));
    let unit = s.trim().chars().last().ok_or_else(bad)?;
    let text = s.trim();
    let num: i64 = text[..text.len() - unit.len_utf8()]
        .parse()
        .map_err(|_| bad())?;
    if num <= 0 {
        return Err(bad());
    }
    match unit {
        'm' => Ok(Duration::minutes(num)),
        'h' => Ok(Duration::hours(num)),
        'd' => Ok(Duration::days(num)),
        _ => Err(bad()),
    }
}

/// Validate the configured jobs. With none configured, the default is `index` every hour.
///
/// # Errors
///
/// - `ConfigError` for a bad interval, an unknown action, or a duplicate name
pub fn jobs_from_config(cfg: &ScheduleConfig) -> Result<Vec<Job>> {
    if cfg.jobs.is_empty() {
        return Ok(vec![Job {
            name: "index".into(),
            every: Duration::hours(1),
            action: Action::Index,
        }]);
    }
    let mut seen = std::collections::HashSet::new();
    let mut jobs = Vec::with_capacity(cfg.jobs.len());
    for spec in &cfg.jobs {
        if spec.name.trim().is_empty() {
            return Err(MemoryError::ConfigError(
                "a schedule job needs a name".into(),
            ));
        }
        if !seen.insert(spec.name.clone()) {
            return Err(MemoryError::ConfigError(format!(
                "duplicate schedule job name '{}'",
                spec.name
            )));
        }
        jobs.push(Job {
            name: spec.name.clone(),
            every: parse_every(&spec.every)?,
            action: Action::parse(&spec.action)?,
        });
    }
    Ok(jobs)
}

/// A job is due when it never ran or its interval has passed since the last run.
pub fn is_due(last_run: Option<DateTime<Utc>>, every: Duration, now: DateTime<Utc>) -> bool {
    last_run.is_none_or(|t| t + every <= now)
}

/// One finished job, as written to the log.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// Job name.
    pub job: String,
    /// Whether the action succeeded.
    pub ok: bool,
    /// One-line summary of what happened.
    pub summary: String,
}

impl Outcome {
    /// `<time> job=<name> outcome=<ok|failed> <summary>`, always one line.
    pub fn line(&self, now: DateTime<Utc>) -> String {
        let summary = self
            .summary
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "{} job={} outcome={} {}",
            now.to_rfc3339(),
            self.job,
            if self.ok { "ok" } else { "failed" },
            summary
        )
    }
}

/// `<root>\Plaesy\schedule.log`, beside `settings.json`. `None` when no per-user root exists.
pub fn log_path() -> Option<PathBuf> {
    settings::default_path().and_then(|p| p.parent().map(|d| d.join("schedule.log")))
}

fn db_err(e: rusqlite::Error) -> MemoryError {
    MemoryError::DatabaseError(e.to_string())
}

fn last_run(engine: &SearchEngine, job: &str) -> Result<Option<DateTime<Utc>>> {
    let db = engine.database();
    let conn = db.conn();
    conn.execute_batch(STATE_SCHEMA).map_err(db_err)?;
    let stored: Option<String> = conn
        .query_row(
            "SELECT last_run FROM schedule_runs WHERE job = ?1",
            params![job],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_err)?;
    Ok(stored.and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|t| t.with_timezone(&Utc))
    }))
}

fn record(engine: &SearchEngine, job: &str, at: DateTime<Utc>, outcome: &str) -> Result<()> {
    let db = engine.database();
    let conn = db.conn();
    conn.execute_batch(STATE_SCHEMA).map_err(db_err)?;
    conn.execute(
        "INSERT INTO schedule_runs (job, last_run, outcome) VALUES (?1, ?2, ?3)
         ON CONFLICT(job) DO UPDATE SET last_run = excluded.last_run, outcome = excluded.outcome",
        params![job, at.to_rfc3339(), outcome],
    )
    .map_err(db_err)?;
    Ok(())
}

async fn run_action(engine: &SearchEngine, root: &Path, action: Action) -> Result<String> {
    // A missing vault must fail the job; indexing a non-existent folder would report success.
    if !root.is_dir() {
        return Err(MemoryError::ConfigError(format!(
            "not a directory: {}",
            root.display()
        )));
    }
    match action {
        Action::Index => {
            let s = engine.index_directory(root).await?;
            Ok(format!(
                "indexed={} unchanged={} removed={} failed={}",
                s.indexed,
                s.unchanged,
                s.removed,
                s.failed.len()
            ))
        }
        Action::Hygiene => {
            let r = engine
                .hygiene(&HygieneOptions {
                    root: Some(root.to_path_buf()),
                    ..HygieneOptions::default()
                })
                .await?;
            Ok(format!(
                "checked={} findings={}",
                r.checked,
                r.findings.len()
            ))
        }
        Action::Missing => {
            let g = engine.missing(Some(root.to_path_buf()), 20).await?;
            Ok(format!(
                "query_gaps={} structural={}",
                g.query_gaps.len(),
                g.structural.len()
            ))
        }
        Action::Curate => {
            let indexed = if WriteLock::daemon_is_active(root) {
                None
            } else {
                Some(engine.index_directory(root).await?)
            };
            let report = engine
                .curate(
                    &HygieneOptions {
                        root: Some(root.to_path_buf()),
                        ..HygieneOptions::default()
                    },
                    20,
                )
                .await?;
            Ok(format!(
                "index={} checked={} findings={} query_gaps={} duplicates={} unlinked={} topic_indexes={}",
                indexed
                    .as_ref()
                    .map(|stats| stats.indexed.to_string())
                    .unwrap_or_else(|| "daemon".to_string()),
                report.checked,
                report.hygiene_findings_total,
                report.query_gaps.items.len(),
                report.exact_duplicates.items.len(),
                report.unlinked_notes.items.len(),
                report.missing_topic_indexes.items.len()
            ))
        }
    }
}

fn append_log(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{line}")
}

/// Run every job that is due at `now`, in config order. Each outcome is logged when `log` is set.
/// A failing job does not stop the others.
///
/// # Errors
///
/// - `DatabaseError` when the run state cannot be read or written
pub async fn run_due(
    engine: &SearchEngine,
    root: &Path,
    jobs: &[Job],
    now: DateTime<Utc>,
    log: Option<&Path>,
) -> Result<Vec<Outcome>> {
    let mut outcomes = Vec::new();
    for job in jobs {
        if !is_due(last_run(engine, &job.name)?, job.every, now) {
            continue;
        }
        // Claim the job before running it, so an overlapping run sees it as not due.
        record(engine, &job.name, now, "running")?;
        let (ok, summary) = match run_action(engine, root, job.action).await {
            Ok(s) => (true, s),
            Err(e) => (false, e.to_string()),
        };
        record(engine, &job.name, now, if ok { "ok" } else { "failed" })?;
        let outcome = Outcome {
            job: job.name.clone(),
            ok,
            summary,
        };
        if let Some(path) = log {
            // A log that cannot be written must not fail the job.
            if let Err(e) = append_log(path, &outcome.line(now)) {
                eprintln!("warning: cannot write {}: {e}", path.display());
            }
        }
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

/// Time until the next job is due, clamped to `[1 s, MAX_SLEEP]`.
fn sleep_for(
    engine: &SearchEngine,
    jobs: &[Job],
    now: DateTime<Utc>,
) -> Result<std::time::Duration> {
    let mut wait = MAX_SLEEP;
    for job in jobs {
        let due_at = last_run(engine, &job.name)?.map_or(now, |t| t + job.every);
        if let Ok(d) = (due_at - now).to_std() {
            wait = wait.min(d);
        } else {
            wait = std::time::Duration::from_secs(1);
        }
    }
    Ok(wait.max(std::time::Duration::from_secs(1)))
}

/// Keep running due jobs until Ctrl-C. Errors are printed and the loop continues.
///
/// # Errors
///
/// - `IoError` only if the terminal cannot be written
pub async fn run_loop(
    engine: &SearchEngine,
    root: &Path,
    jobs: &[Job],
    log: Option<&Path>,
) -> Result<()> {
    loop {
        let now = Utc::now();
        match run_due(engine, root, jobs, now, log).await {
            Ok(outcomes) => {
                for o in outcomes {
                    println!("{}", o.line(now));
                }
            }
            Err(e) => eprintln!("schedule: {e}"),
        }
        let wait = sleep_for(engine, jobs, Utc::now()).unwrap_or(MAX_SLEEP);
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "schedule_test.rs"]
mod schedule_test;
