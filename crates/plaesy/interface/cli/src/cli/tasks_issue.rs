//! Read-only issue tracker view, including derived dependency blocking.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::NaiveDate;

use super::CliResult;
use crate::tasks::{self, issue::Issue, Status};

/// Arguments for `plaesy tasks children`.
#[derive(Debug, clap::Args)]
pub struct ChildrenArgs {
    /// Parent issue ID whose direct children to list
    pub parent: String,
}

/// Direct children of `parent`, in ID order. Grandchildren are not included.
fn direct_children<'a>(issues: &'a BTreeMap<String, Issue>, parent: &str) -> Vec<&'a Issue> {
    issues
        .values()
        .filter(|issue| issue.parent.as_deref() == Some(parent))
        .collect()
}

/// Lists the direct child issues of one parent with their type, lifecycle status, and title.
pub fn children(args: &ChildrenArgs, tasks_dir: &Path) -> CliResult {
    let issues = Issue::load_collection(tasks_dir)?;
    let problems = Issue::validate_collection(tasks_dir)?;
    if !problems.is_empty() {
        return Err(format!("invalid issue collection: {}", problems.join("; ")).into());
    }
    if !issues.contains_key(&args.parent) {
        return Err(format!("issue not found: {}", args.parent).into());
    }
    let refs = tasks::list_all_tasks(tasks_dir)?;
    let found = direct_children(&issues, &args.parent);
    println!("TYPE     STATUS     ID                                       TITLE");
    for child in &found {
        let title = refs
            .iter()
            .find(|task| task.name == child.id)
            .map_or("", |task| task.title.as_str());
        println!(
            "{:<8} {:<10} {:<40} {}",
            child.kind, child.status, child.id, title
        );
    }
    println!("{} child issue(s) of {}", found.len(), args.parent);
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Restrict results to a spec
    #[arg(long)]
    pub spec: Option<String>,
    /// Filter by type (issue, story, bug, epic, task, subtask)
    #[arg(long = "type")]
    pub kind: Option<String>,
    /// Filter by label (repeatable; all requested labels must match)
    #[arg(long = "label")]
    pub labels: Vec<String>,
    /// Filter by assignee
    #[arg(long)]
    pub assignee: Option<String>,
    /// Filter by milestone
    #[arg(long)]
    pub milestone: Option<String>,
    /// Filter by lifecycle status
    #[arg(long)]
    pub status: Option<String>,
    /// Show dependency details and derived blocked/ready state
    #[arg(long)]
    pub dependencies: bool,
    /// Show issues whose due date is on or before this date (YYYY-MM-DD)
    #[arg(long = "due-before")]
    pub due_before: Option<String>,
    /// Show issues past their due date that are not done
    #[arg(long)]
    pub overdue: bool,
    /// Show issues whose ID or title contains this text (ignores case)
    #[arg(long)]
    pub search: Option<String>,
}

/// True when the issue ID or title contains `needle`, ignoring case.
fn matches_search(id: &str, title: &str, needle: &str) -> bool {
    let needle = needle.to_lowercase();
    id.to_lowercase().contains(&needle) || title.to_lowercase().contains(&needle)
}

#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    /// Stable task folder name (issue ID)
    pub issue: String,
    /// Replace issue type
    #[arg(long = "type")]
    pub kind: Option<String>,
    /// Set parent; use --clear-parent to remove it
    #[arg(long, conflicts_with = "clear_parent")]
    pub parent: Option<String>,
    /// Remove the existing parent
    #[arg(long)]
    pub clear_parent: bool,
    /// Replace dependencies (repeatable); --clear-deps removes all
    #[arg(long = "dep", conflicts_with = "clear_deps")]
    pub deps: Vec<String>,
    #[arg(long)]
    pub clear_deps: bool,
    /// Replace labels (repeatable); --clear-labels removes all
    #[arg(long = "label", conflicts_with = "clear_labels")]
    pub labels: Vec<String>,
    #[arg(long)]
    pub clear_labels: bool,
    /// Set assignee; use --clear-assignee to remove it
    #[arg(long, conflicts_with = "clear_assignee")]
    pub assignee: Option<String>,
    #[arg(long)]
    pub clear_assignee: bool,
    /// Set milestone; use --clear-milestone to remove it
    #[arg(long, conflicts_with = "clear_milestone")]
    pub milestone: Option<String>,
    #[arg(long)]
    pub clear_milestone: bool,
    /// Set start date (YYYY-MM-DD); use --clear-start to remove it
    #[arg(long, conflicts_with = "clear_start")]
    pub start: Option<String>,
    #[arg(long)]
    pub clear_start: bool,
    /// Set due date (YYYY-MM-DD); use --clear-due to remove it
    #[arg(long, conflicts_with = "clear_due")]
    pub due: Option<String>,
    #[arg(long)]
    pub clear_due: bool,
    /// Set planned effort (e.g. 90m, 2h, 1h30m); use --clear-estimate to remove it
    #[arg(long, conflicts_with = "clear_estimate")]
    pub estimate: Option<String>,
    #[arg(long)]
    pub clear_estimate: bool,
}

/// Parses a `YYYY-MM-DD` date argument, naming the flag in the error.
fn parse_date(field: &str, value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("{field} must be a date in YYYY-MM-DD form: {value:?}"))
}

/// True when an issue passes the due-date filters. `today` is passed in so tests can fix it.
fn matches_due(
    issue: &Issue,
    status: Status,
    due_before: Option<NaiveDate>,
    overdue: bool,
    today: NaiveDate,
) -> bool {
    if let Some(limit) = due_before {
        if !issue.due_date.is_some_and(|due| due <= limit) {
            return false;
        }
    }
    if overdue && (status == Status::Done || !issue.due_date.is_some_and(|due| due < today)) {
        return false;
    }
    true
}

/// Update only explicitly supplied metadata and validate the whole collection atomically.
pub fn update(args: &UpdateArgs, tasks_dir: &Path) -> CliResult {
    for (field, value) in [
        ("assignee", args.assignee.as_deref()),
        ("milestone", args.milestone.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(format!("{field} cannot be empty").into());
        }
    }
    for label in &args.labels {
        tasks::issue::validate_label(label)?;
    }
    let unique_labels: std::collections::BTreeSet<_> = args.labels.iter().collect();
    if unique_labels.len() != args.labels.len() {
        return Err("duplicate labels".into());
    }

    let start = args
        .start
        .as_deref()
        .map(|value| parse_date("start date", value))
        .transpose()?;
    let due = args
        .due
        .as_deref()
        .map(|value| parse_date("due date", value))
        .transpose()?;
    let estimate = args
        .estimate
        .as_deref()
        .map(|value| tasks::issue::parse_duration_minutes(value).map_err(|error| format!("estimate: {error}")))
        .transpose()?;
    let graph_lock = tasks::issue::IssueGraphLock::acquire(tasks_dir)?;
    let mut issues = Issue::load_collection(tasks_dir)?;
    let mut issue = issues
        .remove(&args.issue)
        .ok_or_else(|| format!("issue not found: {}", args.issue))?;
    if let Some(kind) = &args.kind {
        issue.kind = kind.clone();
    }
    if args.clear_parent {
        issue.parent = None;
    } else if let Some(parent) = &args.parent {
        issue.parent = Some(parent.clone());
    }
    if args.clear_deps {
        issue.depends_on.clear();
    } else if !args.deps.is_empty() {
        issue.depends_on.clone_from(&args.deps);
    }
    if args.clear_labels {
        issue.labels.clear();
    } else if !args.labels.is_empty() {
        issue.labels.clone_from(&args.labels);
    }
    if args.clear_assignee {
        issue.assignee = None;
    } else if let Some(value) = &args.assignee {
        issue.assignee = Some(value.clone());
    }
    if args.clear_milestone {
        issue.milestone = None;
    } else if let Some(value) = &args.milestone {
        issue.milestone = Some(value.clone());
    }
    if args.clear_start {
        issue.start_date = None;
    } else if let Some(value) = start {
        issue.start_date = Some(value);
    }
    if args.clear_due {
        issue.due_date = None;
    } else if let Some(value) = due {
        issue.due_date = Some(value);
    }
    if args.clear_estimate {
        issue.estimate_minutes = None;
    } else if let Some(value) = estimate {
        issue.estimate_minutes = Some(value);
    }
    save_validated(&graph_lock, tasks_dir, &issue)?;
    println!("[✓] Updated issue {}", issue.id);
    Ok(())
}

/// Writes one issue's metadata under the held graph lock, then validates the whole collection.
/// On any problem the previous `issue.json` bytes are restored before the error is returned.
pub(super) fn save_validated(
    graph_lock: &tasks::issue::IssueGraphLock,
    tasks_dir: &Path,
    issue: &Issue,
) -> CliResult {
    let location = tasks::find_task(tasks_dir, &issue.id)
        .ok_or_else(|| format!("issue not found: {}", issue.id))?;
    let task_dir = tasks_dir.join(location.0.as_str()).join(&location.1);
    let old = tasks::issue::read_metadata_bytes(&task_dir)?;
    graph_lock.write_metadata(&task_dir, issue)?;
    let problems = match Issue::validate_collection(tasks_dir) {
        Ok(problems) => problems,
        Err(error) => {
            rollback_metadata(graph_lock, &task_dir, old.as_deref())?;
            return Err(error.into());
        }
    };
    if !problems.is_empty() {
        rollback_metadata(graph_lock, &task_dir, old.as_deref())?;
        return Err(format!("issue update rejected: {}", problems.join("; ")).into());
    }
    Ok(())
}

fn rollback_metadata(
    graph_lock: &tasks::issue::IssueGraphLock,
    task_dir: &Path,
    old: Option<&[u8]>,
) -> Result<(), Box<dyn std::error::Error>> {
    match old {
        Some(bytes) => {
            let issue: Issue = serde_json::from_slice(bytes)?;
            graph_lock
                .write_metadata(task_dir, &issue)
                .map_err(std::io::Error::other)?;
            Ok(())
        }
        None => {
            let path = task_dir.join("issue.json");
            match fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            }
        }
    }
}

pub fn run(args: &Args, tasks_dir: &Path) -> CliResult {
    if let Some(status) = &args.status {
        status.parse::<Status>()?;
    }
    for label in &args.labels {
        tasks::issue::validate_label(label)?;
    }
    let due_before = args
        .due_before
        .as_deref()
        .map(|value| parse_date("--due-before", value))
        .transpose()?;
    if args
        .search
        .as_deref()
        .is_some_and(|needle| needle.trim().is_empty())
    {
        return Err("--search cannot be empty".into());
    }
    let today = chrono::Local::now().date_naive();
    let issues = Issue::load_collection(tasks_dir)?;
    let problems = Issue::validate_collection(tasks_dir)?;
    if !problems.is_empty() {
        return Err(format!("invalid issue collection: {}", problems.join("; ")).into());
    }
    let refs = tasks::list_all_tasks(tasks_dir)?;
    let mut records = Vec::new();
    for task in refs {
        let task_dir = tasks_dir.join(task.status.as_str()).join(&task.name);
        let issue = issues
            .get(&task.name)
            .cloned()
            .ok_or_else(|| format!("missing issue metadata for task {}", task.name))?;
        let frontmatter = tasks::read_frontmatter(&task_dir.join(tasks::TASK_FILE));
        records.push((task, issue, frontmatter));
    }
    let completed: std::collections::BTreeSet<String> = records
        .iter()
        .filter(|(task, _, _)| task.status == Status::Done)
        .map(|(_, issue, _)| issue.id.clone())
        .collect();
    println!("TYPE     STATUS     PRIORITY   ASSIGNEE     MILESTONE    DUE        LABELS      TITLE");
    let mut count = 0;
    for (task, issue, frontmatter) in records {
        if !matches_due(&issue, task.status, due_before, args.overdue, today) {
            continue;
        }
        if args
            .search
            .as_deref()
            .is_some_and(|needle| !matches_search(&issue.id, &task.title, needle))
        {
            continue;
        }
        if args
            .spec
            .as_ref()
            .is_some_and(|spec| frontmatter.get("spec") != Some(spec))
            || args.kind.as_ref().is_some_and(|kind| &issue.kind != kind)
            || !issue.has_labels(&args.labels)
            || args
                .assignee
                .as_ref()
                .is_some_and(|assignee| issue.assignee.as_ref() != Some(assignee))
            || args
                .milestone
                .as_ref()
                .is_some_and(|milestone| issue.milestone.as_ref() != Some(milestone))
            || args
                .status
                .as_ref()
                .is_some_and(|status| task.status.as_str() != status)
        {
            continue;
        }
        count += 1;
        println!(
            "{:<8} {:<10} {:<10} {:<12} {:<12} {:<10} {:<11} {}",
            issue.kind,
            task.status,
            task.name.split('_').next().unwrap_or(""),
            issue.assignee.as_deref().unwrap_or("-"),
            issue.milestone.as_deref().unwrap_or("-"),
            issue.due_date.map_or("-".to_owned(), |due| due.to_string()),
            issue.labels.join(","),
            task.title
        );
        if args.dependencies {
            let waiting: Vec<_> = issue
                .depends_on
                .iter()
                .filter(|dep| !completed.contains(dep.as_str()))
                .collect();
            let readiness = if waiting.is_empty() {
                "ready"
            } else {
                "blocked"
            };
            println!(
                "  {readiness}; depends on: {}",
                if issue.depends_on.is_empty() {
                    "none".into()
                } else {
                    issue.depends_on.join(", ")
                }
            );
            if !waiting.is_empty() {
                println!(
                    "  waiting for: {}",
                    waiting
                        .into_iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }
    println!("{count} issue(s)");
    Ok(())
}

#[cfg(test)]
#[path = "tasks_issue_test.rs"]
mod tasks_issue_test;
