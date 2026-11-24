//! `plaesy dashboard`: writes one static HTML page with task counts per status and delegate usage
//! totals. The page has no scripts, no stylesheet links, and no network references, so it opens
//! offline. Every value from disk is escaped before it is placed in the page.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use thiserror::Error;

use super::CliResult;
use crate::delegate::usage::{self, Record, Totals};
use crate::tasks::{self, Status, TaskRef, STATUSES};

/// File name written under the analysis folder when `--out` is not given.
const DEFAULT_FILE: &str = "dashboard.html";

/// Writes a static HTML dashboard of the project's tasks and delegate usage.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Directory inside the project; the nearest ancestor with `.plaesy/` is used
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Output file (default: `.plaesy/analysis/dashboard.html` under the project root)
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// Errors while collecting data for or writing the dashboard.
#[derive(Debug, Error)]
pub enum DashboardError {
    #[error(transparent)]
    Tasks(#[from] tasks::TasksError),
    #[error("reading delegate usage {}: {source}", path.display())]
    Usage {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("writing dashboard {}: {source}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Everything the page shows, collected before rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Name of the project folder (not the full path, so the page does not leak local paths).
    pub project: String,
    /// RFC 3339, UTC.
    pub generated_at: String,
    /// One entry per status, in lifecycle order, including statuses with no tasks.
    pub task_counts: Vec<(Status, usize)>,
    /// `None` when no delegate call has been recorded.
    pub usage: Option<UsageSummary>,
}

/// Delegate usage totals: all providers together, and per provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageSummary {
    pub total: Totals,
    pub by_provider: BTreeMap<String, Totals>,
}

/// Runs the command: reads the project, renders the page, and writes it.
///
/// # Errors
///
/// No project root is found, the task or usage files cannot be read, or the output cannot be written.
pub fn run(args: &Args) -> CliResult {
    let start = env::current_dir()?.join(&args.path);
    let root = tasks::find_project_root(&start).map_err(DashboardError::from)?;
    let tasks_dir = tasks::tasks_dir(&root);
    let refs = tasks::list_all_tasks(&tasks_dir).map_err(DashboardError::from)?;

    let analysis = root.join(".plaesy").join("analysis");
    let usage_path = usage::usage_path(&analysis);
    let records = usage::read(&usage_path).map_err(|source| DashboardError::Usage {
        path: usage_path.clone(),
        source,
    })?;

    let snapshot = Snapshot {
        project: folder_name(&root),
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        task_counts: count_tasks(&refs),
        usage: summarize_usage(&records),
    };
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| analysis.join(DEFAULT_FILE));
    write_dashboard(&out, &render_html(&snapshot))?;
    println!("wrote {}", out.display());
    Ok(())
}

fn folder_name(root: &Path) -> String {
    root.file_name()
        .map_or_else(|| root.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Number of tasks in each status, in lifecycle order. Statuses with no tasks report zero.
#[must_use]
pub fn count_tasks(refs: &[TaskRef]) -> Vec<(Status, usize)> {
    STATUSES
        .into_iter()
        .map(|status| {
            let count = refs.iter().filter(|r| r.status == status).count();
            (status, count)
        })
        .collect()
}

/// Totals over the usage records, or `None` when there are none.
#[must_use]
pub fn summarize_usage(records: &[Record]) -> Option<UsageSummary> {
    if records.is_empty() {
        return None;
    }
    let (by_provider, _by_chain) = usage::summarize(records);
    let mut total = Totals::default();
    for t in by_provider.values() {
        total.calls += t.calls;
        total.input_tokens += t.input_tokens;
        total.output_tokens += t.output_tokens;
        total.unknown_tokens += t.unknown_tokens;
        total.wall_ms += t.wall_ms;
    }
    Some(UsageSummary { total, by_provider })
}

/// Escapes the five characters that matter in HTML text and attribute values.
#[must_use]
pub fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

const STYLE: &str = "\
:root { color-scheme: light dark; --fg: #1d1f24; --muted: #6b7180; --line: #d9dce3; --bg: #ffffff; }
@media (prefers-color-scheme: dark) { :root { --fg: #e6e8ee; --muted: #9aa1b0; --line: #3a3f4b; --bg: #15171c; } }
body { margin: 0; padding: 16px; background: var(--bg); color: var(--fg); font: 15px/1.5 system-ui, sans-serif; }
main { max-width: 720px; margin: 0 auto; }
h1 { font-size: 1.4em; margin: 0 0 4px; }
h2 { font-size: 1.05em; margin: 24px 0 8px; }
.meta { color: var(--muted); font-size: 0.9em; margin: 0 0 8px; }
table { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
th, td { text-align: left; padding: 6px 8px; border-bottom: 1px solid var(--line); }
th.num, td.num { text-align: right; }
tfoot td { font-weight: 600; }
";

/// Renders the page. The result is a complete HTML document with inline styles only.
#[must_use]
pub fn render_html(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\n",
    );
    out.push_str("<title>Plaesy dashboard</title>\n<style>\n");
    out.push_str(STYLE);
    out.push_str("</style>\n</head>\n<body>\n<main>\n");
    out.push_str(&format!(
        "<h1>Plaesy dashboard: {}</h1>\n",
        escape_html(&snapshot.project)
    ));
    out.push_str(&format!(
        "<p class=\"meta\">Generated {}</p>\n",
        escape_html(&snapshot.generated_at)
    ));

    out.push_str("<h2>Tasks by status</h2>\n<table>\n<thead><tr><th>Status</th><th class=\"num\">Tasks</th></tr></thead>\n<tbody>\n");
    let mut total_tasks = 0usize;
    for (status, count) in &snapshot.task_counts {
        total_tasks += count;
        out.push_str(&format!(
            "<tr><td>{}</td><td class=\"num\">{count}</td></tr>\n",
            escape_html(status.as_str())
        ));
    }
    out.push_str(&format!(
        "</tbody>\n<tfoot><tr><td>Total</td><td class=\"num\">{total_tasks}</td></tr></tfoot>\n</table>\n"
    ));

    out.push_str("<h2>Delegate usage</h2>\n");
    match &snapshot.usage {
        None => out.push_str("<p>No delegate usage recorded.</p>\n"),
        Some(summary) => render_usage(&mut out, summary),
    }

    out.push_str("</main>\n</body>\n</html>\n");
    out
}

fn render_usage(out: &mut String, summary: &UsageSummary) {
    out.push_str(
        "<table>\n<thead><tr><th>Provider</th><th class=\"num\">Calls</th>\
<th class=\"num\">Input tokens</th><th class=\"num\">Output tokens</th>\
<th class=\"num\">Calls without token counts</th><th class=\"num\">Wall time (ms)</th></tr></thead>\n<tbody>\n",
    );
    for (provider, t) in &summary.by_provider {
        push_usage_row(out, &escape_html(provider), t);
    }
    out.push_str("</tbody>\n<tfoot>\n");
    push_usage_row(out, "Total", &summary.total);
    out.push_str("</tfoot>\n</table>\n");
}

fn push_usage_row(out: &mut String, label: &str, t: &Totals) {
    out.push_str(&format!(
        "<tr><td>{label}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>\n",
        t.calls, t.input_tokens, t.output_tokens, t.unknown_tokens, t.wall_ms
    ));
}

fn write_dashboard(path: &Path, html: &str) -> Result<(), DashboardError> {
    let wrap = |source| DashboardError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(wrap)?;
    }
    fs::write(path, html).map_err(wrap)
}

#[cfg(test)]
#[path = "dashboard_test.rs"]
mod dashboard_test;
