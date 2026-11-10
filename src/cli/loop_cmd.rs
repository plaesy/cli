//! `plaesy loop`: runs the autonomous loop of the project until a Completion or Stop report is
//! written. `loop stop-hook` is the Stop hook entry; only the hook runs it.

use std::io;
use std::path::PathBuf;
use std::process::Command;

use chrono::Utc;

use super::common::{find_root, run_bash};
use super::CliResult;
use crate::loop_run::config as loop_config;
use crate::loop_run::driver::{self, Driver, Options};
use crate::loop_run::measure::{measure, Measurement};
use crate::loop_run::step_adapter::{new_step, process_env};
use crate::loop_run::stophook::{exit_code, LOOP_REPORT_DIR};

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<LoopCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum LoopCommand {
    /// Stop hook: keep the turn going (exit 2) while the loop is running
    #[command(hide = true)]
    StopHook,
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        Some(LoopCommand::StopHook) => stop_hook(),
        None => run_loop(),
    }
}

/// Exits 2 while the loop is running, and 0 otherwise.
fn stop_hook() -> CliResult {
    let root = find_root()?;
    let code = exit_code(&root, io::stdin().lock(), &mut io::stderr().lock());
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// The step the `step` object of the config selects, or the `step_command` job when there is none.
fn configured_step(
    root: &std::path::Path,
    cfg: &loop_config::Config,
) -> Result<crate::loop_run::step_adapter::Step, String> {
    let jobs = root.join(LOOP_REPORT_DIR).join("jobs");
    new_step(cfg, root, &jobs, process_env())
}

/// Whether the working tree has uncommitted changes.
fn git_dirty(root: &std::path::Path) -> Result<bool, String> {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    Ok(!String::from_utf8_lossy(&out.stdout).trim().is_empty())
}

fn run_loop() -> CliResult {
    let root: PathBuf = find_root()?;
    let cfg = loop_config::load(&root.join(".plaesy").join("loop.json"))?;
    let step = configured_step(&root, &cfg)?;

    let measure_cfg = cfg.measure.clone();
    let measure_root = root.clone();
    let measure_fn: driver::MeasureFn = Box::new(move |name: &str| -> Measurement {
        let root = measure_root.clone();
        let mut run = move |command: &str| run_bash(&root, command);
        measure(name, &measure_cfg, &mut run)
    });
    let dirty_root = root.clone();
    let dirty: driver::DirtyFn = Box::new(move || git_dirty(&dirty_root));

    let mut d = Driver::new(Options {
        state_path: root.join(".plaesy").join("state.json"),
        report_dir: root.join(LOOP_REPORT_DIR),
        config: cfg,
        step,
        measure: measure_fn,
        dirty: Some(dirty),
        learnings_path: Some(
            root.join(".plaesy")
                .join("memory")
                .join("loop-learnings.md"),
        ),
        out: Box::new(io::stdout()),
        now: Box::new(Utc::now),
    })?;
    d.run()?;
    driver::exit_error(d.status())?;
    Ok(())
}
