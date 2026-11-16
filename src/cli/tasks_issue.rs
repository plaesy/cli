//! Read-only issue tracker view, including derived dependency blocking.

use std::fs;
use std::path::Path;

use super::CliResult;
use crate::tasks::{self, issue::Issue, Status};

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
        if label.trim().is_empty() {
            return Err("labels cannot be empty".into());
        }
    }
    let unique_labels: std::collections::BTreeSet<_> = args.labels.iter().collect();
    if unique_labels.len() != args.labels.len() {
        return Err("duplicate labels".into());
    }

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
    let location = tasks::find_task(tasks_dir, &args.issue)
        .ok_or_else(|| format!("issue not found: {}", args.issue))?;
    let task_dir = tasks_dir.join(location.0.as_str()).join(&location.1);
    let old = tasks::issue::read_metadata_bytes(&task_dir)?;
    graph_lock.write_metadata(&task_dir, &issue)?;
    let problems = match Issue::validate_collection(tasks_dir) {
        Ok(problems) => problems,
        Err(error) => {
            rollback_metadata(&graph_lock, &task_dir, old.as_deref())?;
            return Err(error.into());
        }
    };
    if !problems.is_empty() {
        rollback_metadata(&graph_lock, &task_dir, old.as_deref())?;
        return Err(format!("issue update rejected: {}", problems.join("; ")).into());
    }
    println!("[✓] Updated issue {}", issue.id);
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
    println!("TYPE     STATUS     PRIORITY   ASSIGNEE     MILESTONE    LABELS      TITLE");
    let mut count = 0;
    for (task, issue, frontmatter) in records {
        if args
            .spec
            .as_ref()
            .is_some_and(|spec| frontmatter.get("spec") != Some(spec))
            || args.kind.as_ref().is_some_and(|kind| &issue.kind != kind)
            || !args.labels.iter().all(|label| issue.labels.contains(label))
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
            "{:<8} {:<10} {:<10} {:<12} {:<12} {:<11} {}",
            issue.kind,
            task.status,
            task.name.split('_').next().unwrap_or(""),
            issue.assignee.as_deref().unwrap_or("-"),
            issue.milestone.as_deref().unwrap_or("-"),
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
mod update_tests {
    use super::*;
    use crate::tasks::{
        issue::{self, Issue},
        tasks_dir, TASK_FILE,
    };
    use std::fs;

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let tasks = tasks_dir(root.path());
        for status in crate::tasks::STATUSES {
            fs::create_dir_all(tasks.join(status.as_str())).unwrap();
        }
        for id in ["a", "b"] {
            let folder = tasks.join("todo").join(id);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join(TASK_FILE), "---\ntitle: test\n---\n").unwrap();
            issue::write_metadata_unlocked(
                &folder,
                &Issue {
                    id: id.into(),
                    status: Status::Todo,
                    kind: "task".into(),
                    parent: None,
                    depends_on: vec![],
                    labels: vec!["keep".into()],
                    assignee: Some("owner".into()),
                    milestone: Some("M1".into()),
                },
            )
            .unwrap();
        }
        (root, tasks)
    }

    fn args(id: &str) -> UpdateArgs {
        UpdateArgs {
            issue: id.into(),
            kind: None,
            parent: None,
            clear_parent: false,
            deps: vec![],
            clear_deps: false,
            labels: vec![],
            clear_labels: false,
            assignee: None,
            clear_assignee: false,
            milestone: None,
            clear_milestone: false,
        }
    }

    #[test]
    fn concurrent_updates_preserve_both_fields() {
        use std::thread;

        let (_root, tasks) = fixture();
        let tasks = std::sync::Arc::new(tasks);
        let mut labels = args("a");
        labels.labels = vec!["concurrent".into()];
        let mut assignee = args("a");
        assignee.assignee = Some("new-owner".into());
        let tasks_a = std::sync::Arc::clone(&tasks);
        let tasks_b = std::sync::Arc::clone(&tasks);
        let first = thread::spawn(move || update(&labels, &tasks_a).unwrap());
        let second = thread::spawn(move || update(&assignee, &tasks_b).unwrap());
        first.join().unwrap();
        second.join().unwrap();

        let issue = Issue::load_collection(&tasks).unwrap().remove("a").unwrap();
        assert_eq!(issue.labels, vec!["concurrent"]);
        assert_eq!(issue.assignee.as_deref(), Some("new-owner"));
        assert_eq!(issue.milestone.as_deref(), Some("M1"));
    }

    #[test]
    fn update_preserves_unspecified_fields() {
        let (_root, tasks) = fixture();
        let mut update_args = args("a");
        update_args.labels = vec!["new".into()];
        update(&update_args, &tasks).unwrap();
        let issue = Issue::load_collection(&tasks).unwrap().remove("a").unwrap();
        assert_eq!(issue.labels, vec!["new"]);
        assert_eq!(issue.assignee.as_deref(), Some("owner"));
        assert_eq!(issue.milestone.as_deref(), Some("M1"));
    }

    #[test]
    fn update_rejects_blank_values_and_duplicate_labels_without_changing_metadata() {
        let (_root, tasks) = fixture();
        let path = tasks.join("todo/a/issue.json");
        let original = fs::read(&path).unwrap();

        let mut blank_assignee = args("a");
        blank_assignee.assignee = Some("  ".into());
        assert!(update(&blank_assignee, &tasks).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);

        let mut duplicate_labels = args("a");
        duplicate_labels.labels = vec!["dup".into(), "dup".into()];
        assert!(update(&duplicate_labels, &tasks).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn listing_rejects_dangling_relationships_instead_of_reporting_them_as_waiting() {
        let (_root, tasks) = fixture();
        let mut a = Issue::read(&tasks, crate::tasks::Status::Todo, "a").unwrap();
        a.depends_on = vec!["missing".into()];
        issue::write_metadata_unlocked(&tasks.join("todo/a"), &a).unwrap();

        let result = run(
            &Args {
                spec: None,
                kind: None,
                labels: vec![],
                assignee: None,
                milestone: None,
                status: None,
                dependencies: true,
            },
            &tasks,
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing dependency missing"));
    }

    #[test]
    fn rejected_parent_cycle_restores_original_json() {
        let (_root, tasks) = fixture();
        let path = tasks.join("todo/a/issue.json");
        let mut b = Issue::read(&tasks, crate::tasks::Status::Todo, "b").unwrap();
        b.parent = Some("a".into());
        issue::write_metadata_unlocked(&tasks.join("todo/b"), &b).unwrap();
        let old = fs::read(&path).unwrap();
        let mut update_args = args("a");
        update_args.parent = Some("b".into());
        assert!(update(&update_args, &tasks).is_err());
        assert_eq!(fs::read(path).unwrap(), old);
    }
}
