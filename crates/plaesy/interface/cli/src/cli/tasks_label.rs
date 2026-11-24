//! `plaesy tasks label`: add or remove labels on one issue's `issue.json`.
//!
//! Labels are validated before the file is touched, and the collection is re-validated
//! after the write, so a rejected change leaves the previous metadata in place.

use std::path::Path;

use super::tasks_issue::save_validated;
use super::CliResult;
use crate::tasks::issue::{validate_label, Issue, IssueGraphLock};

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: LabelCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum LabelCommand {
    /// Add labels to an issue; labels it already carries are skipped
    Add {
        /// Stable task folder name (issue ID)
        issue: String,
        /// Label to add (repeatable, at least one): lowercase, no spaces, at most 32 characters
        #[arg(required = true, value_name = "LABEL")]
        labels: Vec<String>,
    },
    /// Remove labels from an issue; labels it does not carry are skipped
    Remove {
        /// Stable task folder name (issue ID)
        issue: String,
        /// Label to remove (repeatable, at least one)
        #[arg(required = true, value_name = "LABEL")]
        labels: Vec<String>,
    },
}

/// Direction of a label change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Add,
    Remove,
}

pub fn run(args: &Args, tasks_dir: &Path) -> CliResult {
    match &args.command {
        LabelCommand::Add { issue, labels } => apply(issue, labels, Change::Add, tasks_dir),
        LabelCommand::Remove { issue, labels } => apply(issue, labels, Change::Remove, tasks_dir),
    }
}

/// Applies `change` for every label to one issue. Every label is validated before the graph
/// lock is taken, so an invalid label fails the whole command without a partial write.
pub fn apply(issue_id: &str, labels: &[String], change: Change, tasks_dir: &Path) -> CliResult {
    for label in labels {
        validate_label(label)?;
    }
    let graph_lock = IssueGraphLock::acquire(tasks_dir)?;
    let mut issues = Issue::load_collection(tasks_dir)?;
    let mut issue = issues
        .remove(issue_id)
        .ok_or_else(|| format!("issue not found: {issue_id}"))?;
    let mut changed = 0usize;
    for label in labels {
        let did_change = match change {
            Change::Add => issue.add_label(label)?,
            Change::Remove => issue.remove_label(label)?,
        };
        if did_change {
            changed += 1;
        }
    }
    if changed == 0 {
        println!("[=] Issue {issue_id}: no label changes");
        return Ok(());
    }
    save_validated(&graph_lock, tasks_dir, &issue)?;
    let shown = if issue.labels.is_empty() {
        "none".to_owned()
    } else {
        issue.labels.join(", ")
    };
    println!("[✓] Issue {issue_id} labels: {shown}");
    Ok(())
}

#[cfg(test)]
#[path = "tasks_label_test.rs"]
mod tasks_label_test;
