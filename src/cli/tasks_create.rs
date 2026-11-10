//! `plaesy tasks create`: creates a task in the backlog.

use std::fs;
use std::path::Path;

use super::CliResult;
use crate::tasks::{self, STATUSES};

/// Creates `.plaesy/tasks/backlog/<priority>_<category>-<title>/` with `task.md` and an empty
/// `contracts.json`. The category and title form the task's topic, so the topic and the folder
/// name share one slug. Priorities: critical, high, medium, low.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// critical, high, medium, or low
    pub priority: String,
    /// Category, for example feature, bug, or chore
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
    let topic = tasks::topic_slug(&args.category, &args.title)?;
    let name = tasks::add_task(tasks_dir, spec, &args.priority, &topic, &args.title)?;
    println!("[✓] Created {name} for spec {spec}");
    Ok(())
}
