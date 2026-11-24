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
    /// Create a subtask (type `subtask`) under an existing parent task
    CreateSubtask(super::tasks_create::SubtaskArgs),
    /// List the direct child issues of a parent task
    Children(super::tasks_issue::ChildrenArgs),
    /// List and filter issues by type, label, assignee, milestone, and status
    Issues(super::tasks_issue::Args),
    /// Update an issue's metadata without changing its stable task ID
    UpdateIssue(super::tasks_issue::UpdateArgs),
    /// Add a comment to a task; @name mentions are stored with it
    Comment { task: String, text: String },
    /// List a task's comments, or remove one with --delete <id>
    Comments {
        task: String,
        /// ID of the comment to remove
        #[arg(long)]
        delete: Option<u64>,
    },
    /// Add or remove labels on one issue (lowercase, no spaces, at most 32 characters)
    Label(super::tasks_label::Args),
    /// List tasks across every status (or --all for the board, --sync-context for context.md)
    List {
        /// Show every spec's tasks as one board, with title and phase, and report integrity problems
        #[arg(long)]
        all: bool,
        /// Rewrite the In-Flight Tasks section of .plaesy/context.md from the task files
        #[arg(long = "sync-context")]
        sync_context: bool,
    },
    /// Show every spec's tasks as text columns, one per status, with counts and short titles
    Board,
    /// List tasks in one status
    ListStatus { status: String },
    /// Pop the next task from backlog into todo
    Next,
    /// Move a task from todo to doing
    Start { task: String },
    /// Move a task from review to done (reviewer decision)
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
    /// Run each task's checklist and record the results; passing tasks move to review
    Validate(super::tasks_validate::Args),
    /// List the saved versions of a task's task.md and issue.json
    History { task: String },
    /// Restore one saved version of a task; the state before the restore is saved first
    Restore { task: String, version: u32 },
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

/// Saves a task's current files before a command changes them. Fails when the
/// snapshot cannot be written, so the change never happens unrecorded.
fn snapshot_before(tasks_dir: &Path, task: &str, verb: &str) -> CliResult {
    tasks::history::snapshot_task(&tasks::project_root_of(tasks_dir), task, verb)?;
    Ok(())
}

fn print_history(tasks_dir: &Path, task: &str) -> CliResult {
    let versions = tasks::history::list_versions(&tasks::project_root_of(tasks_dir), task)?;
    if versions.is_empty() {
        println!("[!] No saved versions of {task}");
        return Ok(());
    }
    println!("History of {task} ({} version(s))", versions.len());
    for version in versions {
        println!(
            "  v{:<4} {:<8} {:<12} {}",
            version.version, version.status, version.created_unix, version.label
        );
    }
    Ok(())
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
        TasksCommand::CreateSubtask(subtask) => {
            super::tasks_create::run_subtask(subtask, &tasks_dir()?, &spec)
        }
        TasksCommand::Children(children) => super::tasks_issue::children(children, &tasks_dir()?),
        TasksCommand::Issues(issues) => super::tasks_issue::run(issues, &tasks_dir()?),
        TasksCommand::Comment { task, text } => {
            super::tasks_comment::add(task, text, &tasks_dir()?)
        }
        TasksCommand::Comments { task, delete } => match delete {
            Some(id) => super::tasks_comment::delete(task, *id, &tasks_dir()?),
            None => super::tasks_comment::list(task, &tasks_dir()?),
        },
        TasksCommand::UpdateIssue(update) => {
            let dir = tasks_dir()?;
            snapshot_before(&dir, &update.issue, "update-issue")?;
            super::tasks_issue::update(update, &dir)
        }
        TasksCommand::Label(label) => super::tasks_label::run(label, &tasks_dir()?),
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
        TasksCommand::Board => super::tasks_board::run(&tasks_dir()?),
        TasksCommand::ListStatus { status } => {
            print_task_list(&tasks_dir()?, parse_status(status)?)
        }
        TasksCommand::Next => {
            let dir = tasks_dir()?;
            if let Some(task) = tasks::next_backlog_task(&dir, &spec)? {
                snapshot_before(&dir, &task, "next")?;
            }
            match tasks::pop_next_task(&dir, &spec)? {
                Some(file) => println!("[✓] Moved: {file} (backlog → todo)"),
                None => println!("[!] No tasks in backlog"),
            }
            Ok(())
        }
        TasksCommand::Start { task } => {
            let dir = tasks_dir()?;
            snapshot_before(&dir, task, "start")?;
            tasks::start_task(&dir, task)?;
            println!("[✓] Moved: {task} (todo → doing)");
            Ok(())
        }
        TasksCommand::Complete { task } => {
            let dir = tasks_dir()?;
            snapshot_before(&dir, task, "complete")?;
            // A task with no checklist items has nothing `validate` could verify.
            let unverified = match tasks::find_task(&dir, task) {
                Some((_, path)) => match crate::checklist::load(&path.join(tasks::CONTRACT_FILE)) {
                    Ok(list) => list.checklist.is_empty(),
                    Err(_) => true,
                },
                None => true,
            };
            tasks::complete_task(&dir, task)?;
            println!("[✓] Moved: {task} (review → done)");
            println!("[✓] Task completed: {task}");
            if unverified {
                println!("[!] Manual completion only; checklist and goal were not verified.");
            } else {
                println!("[!] Reviewer decision recorded by this command; the checklist was verified by `validate`.");
            }
            Ok(())
        }
        TasksCommand::Block { task, reason } => {
            let dir = tasks_dir()?;
            snapshot_before(&dir, task, "block")?;
            tasks::block_task(&dir, task, reason.as_deref().unwrap_or(""))?;
            println!("[✓] Moved: {task} (doing → blocked)");
            Ok(())
        }
        TasksCommand::Unblock { task } => {
            let dir = tasks_dir()?;
            snapshot_before(&dir, task, "unblock")?;
            tasks::unblock_task(&dir, task)?;
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
            let dir = tasks_dir()?;
            snapshot_before(&dir, task, "move")?;
            tasks::move_task(&dir, task, from, to)?;
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
        TasksCommand::History { task } => print_history(&tasks_dir()?, task),
        TasksCommand::Restore { task, version } => {
            let root = tasks::project_root_of(&tasks_dir()?);
            let restored = tasks::history::restore_version(&root, task, *version)?;
            println!(
                "[✓] Restored: {task} to v{} (state before restore saved as v{})",
                restored.version, restored.preserved_as
            );
            Ok(())
        }
    }
}
