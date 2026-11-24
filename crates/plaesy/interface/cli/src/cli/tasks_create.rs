//! `plaesy tasks create`: creates a task in the backlog.

use std::fs;
use std::path::Path;

use super::CliResult;
use crate::tasks::{
    self,
    issue::{self, Issue},
};

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
    /// Issue type: issue, story, bug, epic, task, or subtask
    #[arg(long = "type", default_value = "task")]
    pub kind: String,
    /// Parent task identifier
    #[arg(long)]
    pub parent: Option<String>,
    /// Dependency task identifiers (repeatable)
    #[arg(long = "depends-on")]
    pub depends_on: Vec<String>,
    /// Labels (repeatable)
    #[arg(long = "label")]
    pub labels: Vec<String>,
    /// Assignee handle or name
    #[arg(long)]
    pub assignee: Option<String>,
    /// Milestone name
    #[arg(long)]
    pub milestone: Option<String>,
}

pub fn run(args: &Args, tasks_dir: &Path, spec: &str) -> CliResult {
    if spec.is_empty() {
        return Err("no spec: the current git branch is unknown; pass --spec <folder>".into());
    }
    if !["issue", "story", "bug", "epic", "task", "subtask"].contains(&args.kind.as_str()) {
        return Err(format!("unknown issue type: {:?}", args.kind).into());
    }
    for (field, value) in [
        ("assignee", args.assignee.as_deref()),
        ("milestone", args.milestone.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(format!("{field} cannot be empty").into());
        }
    }
    for label in &args.labels {
        crate::tasks::issue::validate_label(label)?;
    }
    let unique_labels: std::collections::BTreeSet<_> = args.labels.iter().collect();
    if unique_labels.len() != args.labels.len() {
        return Err("duplicate labels".into());
    }
    let graph_lock = issue::IssueGraphLock::acquire(tasks_dir)?;
    let preexisting_problems: std::collections::BTreeSet<_> =
        Issue::validate_collection(tasks_dir)?.into_iter().collect();
    let topic = tasks::topic_slug(&args.category, &args.title)?;
    let expected_name = format!("{}_{}", args.priority, topic);
    if tasks::find_task(tasks_dir, &expected_name).is_some() {
        return Err(format!("task already exists: {expected_name}").into());
    }
    let name = tasks::add_task_locked(tasks_dir, spec, &args.priority, &topic, &args.title)?;
    let status = tasks::Status::Backlog;
    let folder = tasks_dir.join(status.as_str()).join(&name);
    let issue = Issue {
        id: name.clone(),
        status: tasks::Status::Backlog,
        kind: args.kind.clone(),
        parent: args.parent.clone(),
        depends_on: args.depends_on.clone(),
        labels: args.labels.clone(),
        assignee: args.assignee.clone(),
        milestone: args.milestone.clone(),
    };
    if let Err(error) = graph_lock.write_metadata(&folder, &issue) {
        if let Err(cleanup_error) = fs::remove_dir_all(&folder) {
            return Err(format!(
                "writing issue metadata failed: {error}; additionally failed to remove newly created task {}: {cleanup_error}",
                folder.display()
            )
            .into());
        }
        return Err(error.into());
    }
    let new_problems: Vec<_> = Issue::validate_collection(tasks_dir)?
        .into_iter()
        .filter(|problem| !preexisting_problems.contains(problem))
        .collect();
    if !new_problems.is_empty() {
        let error = format!("invalid issue relationships: {}", new_problems.join("; "));
        if let Err(cleanup_error) = fs::remove_dir_all(&folder) {
            return Err(format!(
                "{error}; additionally failed to remove newly created task {}: {cleanup_error}",
                folder.display()
            )
            .into());
        }
        return Err(error.into());
    }
    println!("[✓] Created {name} for spec {spec}");
    Ok(())
}

#[cfg(test)]
#[path = "tasks_create_test.rs"]
mod tasks_create_test;
