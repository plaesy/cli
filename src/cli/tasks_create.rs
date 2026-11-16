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
        if label.trim().is_empty() {
            return Err("labels cannot be empty".into());
        }
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
mod tests {
    use super::*;

    fn args() -> Args {
        Args {
            priority: "high".into(),
            category: "feature".into(),
            title: "New capability".into(),
            kind: "task".into(),
            parent: None,
            depends_on: vec![],
            labels: vec![],
            assignee: None,
            milestone: None,
        }
    }

    #[test]
    fn invalid_metadata_is_rejected_before_task_creation() {
        let root = tempfile::tempdir().unwrap();
        let tasks = tasks::tasks_dir(root.path());
        let mut invalid = args();
        invalid.kind = "invalid".into();

        assert!(run(&invalid, &tasks, "spec").is_err());
        assert!(
            !tasks.exists(),
            "invalid input must not create the task tree"
        );
        assert!(tasks::list_all_tasks(&tasks).unwrap().is_empty());
    }

    #[test]
    fn valid_create_is_not_blocked_by_unrelated_existing_relationship_errors() {
        let root = tempfile::tempdir().unwrap();
        let tasks = tasks::tasks_dir(root.path());
        let existing = args();
        run(&existing, &tasks, "spec").unwrap();
        let existing_name = tasks::list_tasks(&tasks, tasks::Status::Backlog)
            .unwrap()
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let existing_dir = tasks.join("backlog").join(existing_name);
        let mut existing_issue = Issue::read(
            &tasks,
            tasks::Status::Backlog,
            existing_dir.file_name().unwrap().to_str().unwrap(),
        )
        .unwrap();
        existing_issue.depends_on.push("missing-target".into());
        issue::write_metadata_unlocked(&existing_dir, &existing_issue).unwrap();

        let mut next = args();
        next.title = "Another valid task".into();
        assert!(run(&next, &tasks, "spec").is_ok());
        assert_eq!(
            tasks::list_tasks(&tasks, tasks::Status::Backlog)
                .unwrap()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn duplicate_name_does_not_remove_or_overwrite_existing_task() {
        let root = tempfile::tempdir().unwrap();
        let tasks = tasks::tasks_dir(root.path());
        let original = args();
        run(&original, &tasks, "spec").unwrap();
        let task_name = tasks::list_tasks(&tasks, tasks::Status::Backlog)
            .unwrap()
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let task_dir = tasks.join("backlog").join(&task_name);
        let task_md_before = fs::read(task_dir.join(tasks::TASK_FILE)).unwrap();
        let metadata_before = fs::read(task_dir.join("issue.json")).unwrap();

        assert!(run(&original, &tasks, "spec").is_err());
        assert_eq!(
            fs::read(task_dir.join(tasks::TASK_FILE)).unwrap(),
            task_md_before
        );
        assert_eq!(
            fs::read(task_dir.join("issue.json")).unwrap(),
            metadata_before
        );
    }
}
