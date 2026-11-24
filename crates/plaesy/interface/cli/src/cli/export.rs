//! `plaesy export`: writes the task list or the delegate run summary as Markdown or CSV.
//! Rendering lives in `plaesy_common::export`; this file only reads the local project files.

use std::fs;
use std::path::{Path, PathBuf};

use plaesy_common::export::{self, Format, RunRow, TaskRow};

use super::common::find_root;
use super::CliResult;
use crate::delegate::usage;
use crate::tasks::issue::Issue;
use crate::tasks::{self, STATUSES};

/// Exports the task list (`tasks`) or the delegate run summary per provider (`runs`) as Markdown
/// or CSV. Nothing leaves the machine; the output goes to stdout unless `--out` is given.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// What to export: `tasks` or `runs`
    pub what: String,
    /// Output format: `md` or `csv`
    #[arg(long, default_value = "md")]
    pub format: String,
    /// Write to this file instead of stdout
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// Renders the requested export and writes it to stdout or `--out`.
pub fn run(args: &Args) -> CliResult {
    let format: Format = args.format.parse()?;
    let root = find_root()?;
    let text = match args.what.as_str() {
        "tasks" => {
            let rows = task_rows(&tasks::tasks_dir(&root))?;
            export::render_tasks(&rows, format)
        }
        "runs" => {
            let rows = run_rows(&root.join(".plaesy").join("analysis"))?;
            export::render_runs(&rows, format)
        }
        other => return Err(format!("unknown export {other:?} (use tasks or runs)").into()),
    };
    match &args.out {
        Some(path) => fs::write(path, text)?,
        None => print!("{text}"),
    }
    Ok(())
}

/// Every task in every status folder, in lifecycle order then name order.
fn task_rows(tasks_dir: &Path) -> Result<Vec<TaskRow>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    for status in STATUSES {
        let Some(names) = tasks::list_tasks(tasks_dir, status)? else {
            continue;
        };
        for name in names {
            let issue = Issue::read(tasks_dir, status, &name)?;
            rows.push(TaskRow {
                id: issue.id,
                status: status.as_str().to_owned(),
                kind: issue.kind,
                parent: issue.parent,
                depends_on: issue.depends_on,
                labels: issue.labels,
                assignee: issue.assignee,
                milestone: issue.milestone,
            });
        }
    }
    Ok(rows)
}

/// Totals of the delegate usage log, one row per provider.
fn run_rows(analysis_dir: &Path) -> Result<Vec<RunRow>, Box<dyn std::error::Error>> {
    let records = usage::read(&usage::usage_path(analysis_dir))?;
    let (by_provider, _) = usage::summarize(&records);
    Ok(by_provider
        .into_iter()
        .map(|(provider, totals)| RunRow {
            provider,
            calls: totals.calls,
            input_tokens: totals.input_tokens,
            output_tokens: totals.output_tokens,
            calls_without_token_counts: totals.unknown_tokens,
            wall_ms: totals.wall_ms,
        })
        .collect())
}
