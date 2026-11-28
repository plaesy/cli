//! `plaesy tasks time`: log work on one issue and show its estimate against logged time.
//!
//! Entries and the estimate live in `issue.json`. Writes go through the same locked,
//! validated path as `update-issue`, so a rejected change leaves the previous metadata in place.

use std::path::Path;

use chrono::NaiveDate;

use super::tasks_issue::save_validated;
use super::CliResult;
use crate::tasks::issue::{
    format_minutes, parse_duration_minutes, Issue, IssueGraphLock, TimeEntry,
};

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: TimeCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum TimeCommand {
    /// Log work on an issue
    Log {
        /// Stable task folder name (issue ID)
        issue: String,
        /// Time worked: 90m, 2h, or 1h30m
        duration: String,
        /// Day the work was done (YYYY-MM-DD); defaults to today
        #[arg(long)]
        date: Option<String>,
        /// Note describing the work
        #[arg(long)]
        note: Option<String>,
    },
    /// Show the estimate, logged time, and remaining effort of an issue
    Show {
        /// Stable task folder name (issue ID)
        issue: String,
    },
}

pub fn run(args: &Args, tasks_dir: &Path) -> CliResult {
    match &args.command {
        TimeCommand::Log {
            issue,
            duration,
            date,
            note,
        } => log(issue, duration, date.as_deref(), note.as_deref(), tasks_dir),
        TimeCommand::Show { issue } => show(issue, tasks_dir),
    }
}

/// Parses `--date`, or returns today when it is absent.
fn parse_day(value: Option<&str>) -> Result<NaiveDate, String> {
    match value {
        None => Ok(chrono::Local::now().date_naive()),
        Some(text) => NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map_err(|_| format!("--date must be a date in YYYY-MM-DD form: {text:?}")),
    }
}

/// Appends one time entry to an issue. Every argument is checked before the graph lock is taken.
pub fn log(
    issue_id: &str,
    duration: &str,
    date: Option<&str>,
    note: Option<&str>,
    tasks_dir: &Path,
) -> CliResult {
    let minutes = parse_duration_minutes(duration)?;
    let day = parse_day(date)?;
    let note = note
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);

    let graph_lock = IssueGraphLock::acquire(tasks_dir)?;
    let mut issues = Issue::load_collection(tasks_dir)?;
    let mut issue = issues
        .remove(issue_id)
        .ok_or_else(|| format!("issue not found: {issue_id}"))?;
    issue.time_entries.push(TimeEntry {
        date: day,
        minutes,
        note,
    });
    save_validated(&graph_lock, tasks_dir, &issue)?;
    println!(
        "[✓] Logged {} on {} ({} spent in total)",
        format_minutes(u64::from(minutes)),
        issue.id,
        format_minutes(issue.spent_minutes())
    );
    Ok(())
}

/// The remaining or overrun line for an estimate, or an empty string when there is no estimate.
pub fn effort_line(estimate: Option<u32>, spent: u64) -> String {
    match estimate.map(u64::from) {
        None => String::new(),
        Some(estimate) if spent <= estimate => {
            format!("Remaining: {}", format_minutes(estimate - spent))
        }
        Some(estimate) => format!("Over estimate by: {}", format_minutes(spent - estimate)),
    }
}

/// Prints the estimate, the logged total, the remaining or overrun effort, and each entry.
pub fn show(issue_id: &str, tasks_dir: &Path) -> CliResult {
    let issues = Issue::load_collection(tasks_dir)?;
    let problems = Issue::validate_collection(tasks_dir)?;
    if !problems.is_empty() {
        return Err(format!("invalid issue collection: {}", problems.join("; ")).into());
    }
    let issue = issues
        .get(issue_id)
        .ok_or_else(|| format!("issue not found: {issue_id}"))?;
    let spent = issue.spent_minutes();
    println!("Issue {}", issue.id);
    match issue.estimate_minutes.map(u64::from) {
        Some(estimate) => println!("Estimate: {}", format_minutes(estimate)),
        None => println!("Estimate: none"),
    }
    println!("Spent: {}", format_minutes(spent));
    let effort = effort_line(issue.estimate_minutes, spent);
    if !effort.is_empty() {
        println!("{effort}");
    }
    if issue.time_entries.is_empty() {
        println!("No time logged.");
    }
    for entry in &issue.time_entries {
        println!(
            "  {}  {:<8}  {}",
            entry.date,
            format_minutes(u64::from(entry.minutes)),
            entry.note.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "tasks_time_test.rs"]
mod tasks_time_test;
