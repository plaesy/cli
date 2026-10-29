//! `plaesy tasks create`: creates a task in the backlog.

use std::fs;
use std::path::Path;

use super::CliResult;
use crate::tasks::{self, STATUSES};

/// Creates `.plaesy/tasks/backlog/<priority>_<category>-<title>/` with `spec.md` and an empty
/// `contracts.json`. Priorities: critical, high, medium, low.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// critical, high, medium, or low
    pub priority: String,
    /// Category slug
    pub category: String,
    /// Task title
    pub title: String,
}

pub fn run(args: &Args, tasks_dir: &Path, spec: &str) -> CliResult {
    if spec.is_empty() {
        return Err("no spec: the current git branch is unknown; pass --spec <folder>".into());
    }
    for status in STATUSES {
        fs::create_dir_all(tasks_dir.join(status.as_str()))?;
    }
    let name = tasks::add_task(tasks_dir, spec, &args.priority, &args.category, &args.title)?;
    println!("[✓] Created {name} for spec {spec}");
    Ok(())
}
