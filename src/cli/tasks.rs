//! `plaesy tasks`: the task lifecycle under `.plaesy/tasks/` (backlog, todo, doing, done, blocked).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::common::find_root;
use super::CliResult;
use crate::common::git;
use crate::tasks::{self, Status, STATUSES};

/// Acts on the tasks of the spec whose folder ends in the current git branch, or of `--spec`.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Spec to act on (default: the current git branch, which is the spec's name)
    #[arg(long, global = true, default_value = "")]
    pub spec: String,
    #[command(subcommand)]
    pub command: Option<TasksCommand>,
}

#[derive(Debug, clap::Subcommand)]
pub enum TasksCommand {
    /// Create a task in the backlog
    Create(super::tasks_create::Args),
    /// List and filter issues by type, label, assignee, milestone, and status
    Issues(super::tasks_issue::Args),
    /// Update an issue's metadata without changing its stable task ID
    UpdateIssue(super::tasks_issue::UpdateArgs),
    /// List tasks across every status (or --all for the board, --sync-context for context.md)
    List {
        /// Show every spec's tasks as one board, with title and phase, and report integrity problems
        #[arg(long)]
        all: bool,
        /// Rewrite the In-Flight Tasks section of .plaesy/context.md from the task files
        #[arg(long = "sync-context")]
        sync_context: bool,
    },
    /// List tasks in one status
    ListStatus { status: String },
    /// Pop the next task from backlog into todo
    Next,
    /// Move a task from todo to doing
    Start { task: String },
    /// Move a task from doing to done
    Complete { task: String },
    /// Move a task from doing to blocked, recording a reason
    Block {
        task: String,
        reason: Option<String>,
    },
    /// Move a task from blocked back to todo
    Unblock { task: String },
    /// Move a task file between status directories
    Move {
        task: String,
        from_status: String,
        to_status: String,
    },
    /// Show a task's raw content
    Show {
        task: String,
        status: Option<String>,
    },
    /// Report missing paths and links in the spec.md and plan.md of the task's topic
    Check { task: String },
    /// Hand todo tasks of the current spec to a delegate provider or chain
    Run(super::tasks_run::Args),
    /// Run each task's checklist, record the results, and move finished tasks to done
    Validate(super::tasks_validate::Args),
}

/// The `.plaesy/tasks` folder of the project above the working directory.
fn tasks_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(tasks::tasks_dir(&find_root()?))
}

/// The spec a command acts on: `--spec`, or else the current git branch.
pub fn current_spec(spec_flag: &str) -> String {
    if !spec_flag.is_empty() {
        return spec_flag.to_owned();
    }
    git::current_branch().unwrap_or_default()
}

fn parse_status(text: &str) -> Result<Status, Box<dyn std::error::Error>> {
    Status::from_str(text).map_err(|_| format!("unknown status {text:?}").into())
}

fn print_task_list(tasks_dir: &Path, status: Status) -> CliResult {
    match tasks::list_tasks(tasks_dir, status)? {
        None => println!("[!] No tasks in {status}"),
        Some(names) => {
            println!("{status} ({})", names.len());
            for t in names {
                println!("  - {t}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tasks_test.rs"]
mod tasks_test;

fn goal_is_unfilled(task: &str) -> bool {
    goal_summary(task).is_none() && task.lines().map(str::trim).any(|line| line == "## Goal")
}

fn goal_summary(task: &str) -> Option<String> {
    let mut in_goal = false;
    let mut paragraph = Vec::new();
    for line in task.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            if in_goal {
                break;
            }
            in_goal = trimmed == "## Goal";
            continue;
        }
        if in_goal {
            if trimmed.is_empty() {
                if !paragraph.is_empty() {
                    break;
                }
            } else {
                paragraph.push(trimmed);
            }
        }
    }
    let summary = paragraph.join(" ");
    (!summary.is_empty() && summary != "Describe the observable outcome this task should achieve.")
        .then_some(summary)
}

fn print_board(tasks_dir: &Path) -> CliResult {
    let refs = tasks::list_all_tasks(tasks_dir)?;
    println!("📋 Task Board ({} tasks)\n", refs.len());
    let mut current_spec = String::new();
    for r in &refs {
        if r.spec != current_spec {
            current_spec = r.spec.clone();
            println!("{current_spec}");
        }
        println!(
            "  {:<6} {:<40} {:<10} {}",
            r.status.as_str(),
            r.name,
            r.phase,
            r.title
        );
        let task_path = tasks_dir
            .join(r.status.as_str())
            .join(&r.name)
            .join(tasks::TASK_FILE);
        if let Ok(content) = fs::read_to_string(task_path) {
            if let Some(goal) = goal_summary(&content) {
                println!("         Goal: {goal}");
            } else if goal_is_unfilled(&content) {
                println!("         Goal: belum diisi");
            }
        }
    }
    let problems = tasks::check_integrity(tasks_dir)?;
    println!();
    if problems.is_empty() {
        println!("[✓] No integrity problems");
        return Ok(());
    }
    println!("[!] {} integrity problem(s)", problems.len());
    for p in &problems {
        println!("  - {}: {}", p.task, p.message);
    }
    Ok(())
}

fn sync_in_flight() -> CliResult {
    let root = find_root()?;
    let refs = tasks::list_all_tasks(&tasks::tasks_dir(&root))?;
    if tasks::inflight::sync_in_flight(&root, &refs)? {
        println!("[✓] In-Flight Tasks written to .plaesy/context.md");
    } else {
        println!("[✓] In-Flight Tasks already up to date");
    }
    Ok(())
}

/// The content of a task and the status it was found in. `status` narrows the search.
fn read_task(
    tasks_dir: &Path,
    task: &str,
    status: Option<Status>,
) -> Result<(String, Status), Box<dyn std::error::Error>> {
    if std::path::Path::new(task).components().count() != 1
        || !matches!(
            std::path::Path::new(task).components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err(format!("invalid task name: {task}").into());
    }
    let found = match status {
        Some(s) => tasks::find_task(tasks_dir, task)
            .filter(|(found_status, _)| *found_status == s)
            .and_then(|(found_status, task_dir)| {
                let task_file = task_dir.join(tasks::TASK_FILE);
                fs::symlink_metadata(&task_file)
                    .is_ok_and(|metadata| !metadata.file_type().is_symlink())
                    .then_some((found_status, task_file))
            }),
        None => tasks::find_task(tasks_dir, task).and_then(|(status, dir)| {
            let task_file = dir.join(tasks::TASK_FILE);
            fs::symlink_metadata(&task_file)
                .is_ok_and(|metadata| !metadata.file_type().is_symlink())
                .then_some((status, task_file))
        }),
    };
    let (status, path) = found.ok_or_else(|| format!("task not found: {task}"))?;
    Ok((fs::read_to_string(path)?, status))
}

#[cfg(test)]
#[path = "tasks_read_task_tests_test.rs"]
mod tasks_read_task_tests_test;

pub fn run(args: &Args) -> CliResult {
    let Some(command) = &args.command else {
        return super::print_group_help("tasks");
    };
    let spec = current_spec(&args.spec);
    let stdout = io::stdout();
    match command {
        TasksCommand::Create(create) => super::tasks_create::run(create, &tasks_dir()?, &spec),
        TasksCommand::Issues(issues) => super::tasks_issue::run(issues, &tasks_dir()?),
        TasksCommand::UpdateIssue(update) => super::tasks_issue::update(update, &tasks_dir()?),
        TasksCommand::List { all, sync_context } => {
            if *sync_context {
                return sync_in_flight();
            }
            let dir = tasks_dir()?;
            if *all {
                return print_board(&dir);
            }
            println!("📋 Task Summary\n");
            for status in STATUSES {
                print_task_list(&dir, status)?;
                println!();
            }
            Ok(())
        }
        TasksCommand::ListStatus { status } => {
            print_task_list(&tasks_dir()?, parse_status(status)?)
        }
        TasksCommand::Next => {
            let dir = tasks_dir()?;
            match tasks::pop_next_task(&dir, &spec)? {
                Some(file) => println!("[✓] Moved: {file} (backlog → todo)"),
                None => println!("[!] No tasks in backlog"),
            }
            Ok(())
        }
        TasksCommand::Start { task } => {
            tasks::start_task(&tasks_dir()?, task)?;
            println!("[✓] Moved: {task} (todo → doing)");
            Ok(())
        }
        TasksCommand::Complete { task } => {
            tasks::complete_task(&tasks_dir()?, task)?;
            println!("[✓] Moved: {task} (review → done)");
            println!("[✓] Task completed: {task}");
            println!("[!] Reviewer decision recorded by this command; the checklist was verified by `validate`.");
            Ok(())
        }
        TasksCommand::Block { task, reason } => {
            tasks::block_task(&tasks_dir()?, task, reason.as_deref().unwrap_or(""))?;
            println!("[✓] Moved: {task} (doing → blocked)");
            Ok(())
        }
        TasksCommand::Unblock { task } => {
            tasks::unblock_task(&tasks_dir()?, task)?;
            println!("[✓] Moved: {task} (blocked → todo)");
            println!("[✓] Task unblocked: {task}");
            Ok(())
        }
        TasksCommand::Move {
            task,
            from_status,
            to_status,
        } => {
            let (from, to) = (parse_status(from_status)?, parse_status(to_status)?);
            tasks::move_task(&tasks_dir()?, task, from, to)?;
            println!("[✓] Moved: {task} ({from_status} → {to_status})");
            Ok(())
        }
        TasksCommand::Show { task, status } => {
            let dir = tasks_dir()?;
            let status = match status {
                Some(s) => Some(parse_status(s)?),
                None => None,
            };
            let (content, resolved) = read_task(&dir, task, status)?;
            let mut out = stdout.lock();
            writeln!(out, "📄 Task: {task}")?;
            writeln!(out, "Status: {resolved}")?;
            writeln!(out)?;
            write!(out, "{content}")?;
            Ok(())
        }
        TasksCommand::Check { task } => {
            tasks::check::check_task(&tasks_dir()?, task, &mut stdout.lock())?;
            Ok(())
        }
        TasksCommand::Run(run_args) => super::tasks_run::run(run_args, &spec),
        TasksCommand::Validate(validate_args) => {
            super::tasks_validate::run(validate_args, &args.spec)
        }
    }
}
