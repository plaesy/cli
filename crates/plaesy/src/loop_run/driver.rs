//! The loop driver: runs iterations until a Stop or Completion condition holds.
//!
//! The driver resumes from the recorded state, so a restart continues at the next
//! iteration. A run ends with a Completion report when every mandatory threshold
//! is met, or with a Stop report when a cap or the failure limit is reached.

use std::io::{self, Write};
use std::path::PathBuf;

use chrono::{DateTime, SecondsFormat, Utc};
use thiserror::Error;

use super::completion::{read_candidates, unmet_thresholds, write_report};
use super::config::Config;
use super::measure::Measurement;
use super::queue::{Finding, Queue};
use super::state::{self, State, StateError};
use super::step_adapter::{Step, StepResult};
use super::{COMPLETION_REPORT_FILE, STOP_REPORT_FILE};
use crate::scaffold::stophook::LOOP_REPORT_DIR;

/// Errors from a driver run.
#[derive(Debug, Error)]
pub enum DriverError {
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Measures a named threshold.
pub type MeasureFn = Box<dyn FnMut(&str) -> Measurement>;
/// Reports whether the working tree has uncommitted changes.
pub type DirtyFn = Box<dyn FnMut() -> Result<bool, String>>;

/// Everything a driver needs.
pub struct Options {
    pub state_path: PathBuf,
    pub report_dir: PathBuf,
    pub config: Config,
    pub step: Step,
    pub measure: MeasureFn,
    /// `None` means the tree is treated as clean.
    pub dirty: Option<DirtyFn>,
    /// Learnings file whose candidate rules the reports list.
    pub learnings_path: Option<PathBuf>,
    /// Receives progress lines. It never receives a question.
    pub out: Box<dyn Write>,
    /// Stamps the reports and heartbeats.
    pub now: Box<dyn Fn() -> DateTime<Utc>>,
}

/// Runs the loop.
pub struct Driver {
    opt: Options,
    st: State,
    queue: Queue,
    decisions: Vec<String>,
    last_unmet: Vec<String>,
    cap_ended: Option<String>,
}

impl Driver {
    /// Loads the recorded state.
    ///
    /// # Errors
    ///
    /// [`DriverError::State`] when the state file cannot be read.
    pub fn new(opt: Options) -> Result<Self, DriverError> {
        let mut st = state::load(&opt.state_path)?;
        if st.status.is_empty() {
            st.status = "running".into();
        }
        if st.max_consecutive_failures == 0 {
            st.max_consecutive_failures = opt.config.max_consecutive_failures;
        }
        Ok(Self {
            opt,
            st,
            queue: Queue::default(),
            decisions: Vec::new(),
            last_unmet: Vec::new(),
            cap_ended: None,
        })
    }

    /// The status recorded in the state file.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.st.status
    }

    /// Iterates until Completion or Stop, and writes the matching report.
    ///
    /// # Errors
    ///
    /// State, report, or output failures.
    pub fn run(&mut self) -> Result<(), DriverError> {
        if self.st.status != "running" {
            return Ok(());
        }
        if self.st.started_at.is_empty() {
            self.st.started_at = self.now_rfc3339();
        }
        self.beat()?;

        loop {
            let iter = self.st.current_iteration + 1;
            let max = self.opt.config.max_iterations;
            if max > 0 && iter > max {
                return self.finish("paused", &format!("max iterations ({max}) reached"));
            }
            if let Some(reason) = self.cap_stop() {
                self.cap_ended = Some(reason.clone());
                return self.finish("paused", &format!("cap reached: {reason}"));
            }
            self.check_dirty();

            self.beat()?;
            let (result, error) = match (self.opt.step)(iter) {
                Ok(result) => (result, None),
                Err(e) => (StepResult::default(), Some(e)),
            };
            for unknown in &result.unknown {
                self.decide(
                    &format!("unknown fact: {unknown}"),
                    "recorded as UNKNOWN and continued (default)",
                );
            }
            for finding in result.findings {
                self.queue.record(finding);
            }
            if let Some(tokens) = result.tokens {
                self.st.tokens_used += tokens;
                self.st.tokens_reported = true;
            }

            let clean = error.is_none() && result.clean;
            self.st.current_iteration = iter;
            if clean {
                self.st.consecutive_failures = 0;
                self.st.clean_cycles += 1;
                writeln!(self.opt.out, "iteration {iter}: clean")?;
            } else {
                self.st.consecutive_failures += 1;
                self.st.clean_cycles = 0;
                writeln!(
                    self.opt.out,
                    "iteration {iter}: not clean ({})",
                    error.unwrap_or_else(|| "step reported a failure".into())
                )?;
            }
            state::save(&self.opt.state_path, &self.st)?;

            let limit = self.st.max_consecutive_failures;
            if limit > 0 && self.st.consecutive_failures >= limit {
                let n = self.st.consecutive_failures;
                return self.finish("paused", &format!("{n} failed cycles in a row"));
            }
            let need = self.opt.config.clean_cycles_for_completion;
            if need > 0 && self.st.clean_cycles >= need {
                let (unmet, notes) =
                    unmet_thresholds(&self.opt.config.thresholds, &mut *self.opt.measure);
                for note in notes {
                    self.decide(&note, "treated as unmet (default), so no Completion");
                }
                if unmet.is_empty() {
                    return self.finish(
                        "complete",
                        &format!("{need} clean cycles and every mandatory threshold met"),
                    );
                }
                self.last_unmet = unmet;
            }
        }
    }

    fn check_dirty(&mut self) {
        let Some(dirty) = self.opt.dirty.as_mut() else {
            return;
        };
        match dirty() {
            Err(e) => {
                let question = format!("dirty working tree check failed: {e}");
                self.decide(&question, "continued (default)");
            }
            Ok(true) => self.decide(
                "dirty working tree",
                "continued with the changes left in place (default)",
            ),
            Ok(false) => {}
        }
    }

    /// The first hard cap reached, if any. A cap that cannot be measured does not end the run.
    fn cap_stop(&mut self) -> Option<String> {
        let cfg = self.opt.config.clone();
        if cfg.max_run_iterations > 0 && self.st.current_iteration >= cfg.max_run_iterations {
            return Some(format!("max_run_iterations = {}", cfg.max_run_iterations));
        }
        if cfg.max_wall_minutes > 0 {
            if let Some(elapsed) = self.elapsed_minutes() {
                if elapsed >= cfg.max_wall_minutes as f64 {
                    return Some(format!(
                        "max_wall_minutes = {} ({elapsed:.1} minutes elapsed)",
                        cfg.max_wall_minutes
                    ));
                }
            }
        }
        if cfg.max_tokens > 0 && self.st.tokens_reported && self.st.tokens_used >= cfg.max_tokens {
            return Some(format!(
                "max_tokens = {} ({} tokens used)",
                cfg.max_tokens, self.st.tokens_used
            ));
        }
        None
    }

    /// Minutes since `started_at`. `None` when it cannot be read, and that decision is recorded.
    fn elapsed_minutes(&mut self) -> Option<f64> {
        match DateTime::parse_from_rfc3339(&self.st.started_at) {
            Ok(start) => {
                let elapsed = (self.now() - start.with_timezone(&Utc)).num_milliseconds() as f64;
                Some(elapsed / 60_000.0)
            }
            Err(e) => {
                self.decide(
                    &format!("wall-clock cap: started_at unreadable ({e})"),
                    "cap not measured, so it does not end the run (default)",
                );
                None
            }
        }
    }

    fn budget_lines(&mut self) -> Vec<String> {
        let cfg = self.opt.config.clone();
        let ended = self
            .cap_ended
            .clone()
            .unwrap_or_else(|| "none (the run did not reach a cap)".into());
        let wall = match self.elapsed_minutes() {
            Some(elapsed) => format!(
                "{elapsed:.1} minutes elapsed; max_wall_minutes = {}",
                cfg.max_wall_minutes
            ),
            None => "UNKNOWN (started_at unreadable)".into(),
        };
        let tokens = if self.st.tokens_reported {
            format!("{}; max_tokens = {}", self.st.tokens_used, cfg.max_tokens)
        } else {
            format!(
                "UNKNOWN (no step reported a count); max_tokens = {}",
                cfg.max_tokens
            )
        };
        vec![
            format!("Cap that ended the run: {ended}"),
            format!(
                "Iterations: {} of max_run_iterations = {}",
                self.st.current_iteration, cfg.max_run_iterations
            ),
            format!("Wall-clock: {wall}"),
            format!("Tokens used: {tokens}"),
        ]
    }

    /// Records this process as the live driver, with the current time.
    fn beat(&mut self) -> Result<(), DriverError> {
        self.st.driver_pid = i64::from(std::process::id());
        self.st.heartbeat_at = self.now_rfc3339();
        state::save(&self.opt.state_path, &self.st)?;
        Ok(())
    }

    /// Writes the end-of-run report, then saves the status. The report comes first, so a
    /// Stop hook that sees the state as not running also sees the report.
    fn finish(&mut self, status: &str, reason: &str) -> Result<(), DriverError> {
        let name = if status == "complete" {
            COMPLETION_REPORT_FILE
        } else {
            STOP_REPORT_FILE
        };
        // Read the budget first: measuring the clock can record a decision the report must list.
        let budget = self.budget_lines();
        let mut lines = vec![
            format!("# Loop report: {status}"),
            String::new(),
            format!("- Status: {status}"),
            format!("- Reason: {reason}"),
            format!("- Iterations run: {}", self.st.current_iteration),
            format!("- Written: {}", self.now_rfc3339()),
            String::new(),
            "## Decisions made by config defaults (no prompt)".into(),
        ];
        push_or_none(&mut lines, &self.decisions);
        lines.push(String::new());
        lines.push("## Unmet mandatory thresholds".into());
        push_or_none(&mut lines, &self.last_unmet);
        lines.push(String::new());
        lines.push("## Budget (caps and usage)".into());
        lines.extend(budget);
        lines.push(String::new());
        lines.push(format!(
            "## Findings queued and not taken: {}",
            self.queue.len()
        ));
        lines.push(String::new());
        lines.push("## Candidate rules waiting for approval".into());
        let candidates = read_candidates(self.opt.learnings_path.as_deref());
        push_or_none(&mut lines, &candidates);

        write_report(&self.opt.report_dir, name, &lines)?;
        self.st.status = status.to_owned();
        state::save(&self.opt.state_path, &self.st)?;
        writeln!(self.opt.out, "run ended: {status} ({reason})")?;
        Ok(())
    }

    /// Records a decision a config default settled. Each distinct decision is kept once.
    fn decide(&mut self, question: &str, chosen: &str) {
        let entry = format!("{question}: {chosen}");
        if !self.decisions.contains(&entry) {
            self.decisions.push(entry);
        }
    }

    fn now(&self) -> DateTime<Utc> {
        (self.opt.now)()
    }

    fn now_rfc3339(&self) -> String {
        self.now().to_rfc3339_opts(SecondsFormat::Secs, true)
    }

    /// Queues a finding, as a step would. Exposed for adapters that report findings out of band.
    pub fn record_finding(&mut self, finding: Finding) -> bool {
        self.queue.record(finding)
    }
}

fn push_or_none(lines: &mut Vec<String>, items: &[String]) {
    if items.is_empty() {
        lines.push("- none".into());
    }
    lines.extend(items.iter().map(|item| format!("- {item}")));
}

/// Exit decision of `plaesy loop`: a Completion succeeds, and any other status is an error.
///
/// # Errors
///
/// A run that did not complete.
pub fn exit_error(status: &str) -> Result<(), String> {
    if status == "complete" {
        return Ok(());
    }
    Err(format!(
        "loop run did not complete (status {status}); see the Stop Report in {LOOP_REPORT_DIR}"
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::loop_run::measure::Measurement;

    /// A writer that keeps its bytes so a test can read them afterwards.
    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl Write for Capture {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn options(dir: &std::path::Path, step_clean: bool, config: Config) -> (Options, Capture) {
        let out = Capture::default();
        let opt = Options {
            state_path: dir.join("state.json"),
            report_dir: dir.join("reports"),
            config,
            step: Box::new(move |_| {
                Ok(StepResult {
                    clean: step_clean,
                    ..StepResult::default()
                })
            }),
            measure: Box::new(|name| Measurement {
                name: name.to_owned(),
                value: 95.0,
                known: true,
                note: "measured".into(),
            }),
            dirty: None,
            learnings_path: None,
            out: Box::new(out.clone()),
            now: Box::new(Utc::now),
        };
        (opt, out)
    }

    #[test]
    fn clean_cycles_with_met_thresholds_complete_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config {
            clean_cycles_for_completion: 2,
            ..Config::default()
        };
        let (opt, _out) = options(dir.path(), true, config);
        let mut driver = Driver::new(opt).unwrap();
        driver.run().unwrap();
        assert_eq!(driver.status(), "complete");
        assert!(dir
            .path()
            .join("reports")
            .join(COMPLETION_REPORT_FILE)
            .exists());
    }

    #[test]
    fn consecutive_failures_pause_the_run_with_a_stop_report() {
        let dir = tempfile::tempdir().unwrap();
        let (opt, _out) = options(dir.path(), false, Config::default());
        let mut driver = Driver::new(opt).unwrap();
        driver.run().unwrap();
        assert_eq!(driver.status(), "paused");
        assert!(dir.path().join("reports").join(STOP_REPORT_FILE).exists());
    }

    #[test]
    fn a_completed_run_is_an_exit_success_and_a_paused_run_is_not() {
        assert!(exit_error("complete").is_ok());
        assert!(exit_error("paused")
            .unwrap_err()
            .contains("did not complete"));
    }
}
