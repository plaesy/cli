//! Local export of the task list and the run summary as Markdown or CSV.
//!
//! This module only renders text from plain rows. Reading the task folders and the delegate
//! usage log is the caller's job, so nothing here touches the filesystem.

use std::str::FromStr;

/// Output format of an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Csv,
}

/// Errors from parsing an export request.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("unknown export format {0:?} (use md or csv)")]
    UnknownFormat(String),
}

impl FromStr for Format {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "md" | "markdown" => Ok(Self::Markdown),
            "csv" => Ok(Self::Csv),
            other => Err(Error::UnknownFormat(other.to_owned())),
        }
    }
}

/// Column names of a task export, in output order.
pub const TASK_COLUMNS: [&str; 8] = [
    "id",
    "status",
    "type",
    "parent",
    "depends_on",
    "labels",
    "assignee",
    "milestone",
];

/// Column names of a run summary, in output order.
pub const RUN_COLUMNS: [&str; 6] = [
    "provider",
    "calls",
    "input_tokens",
    "output_tokens",
    "calls_without_token_counts",
    "wall_ms",
];

/// One task folder, with the metadata from its `issue.json` (or defaults for legacy folders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    pub id: String,
    pub status: String,
    pub kind: String,
    pub parent: Option<String>,
    pub depends_on: Vec<String>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub milestone: Option<String>,
}

/// Totals of the delegate calls made through one provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    pub provider: String,
    pub calls: u64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub calls_without_token_counts: u64,
    pub wall_ms: i64,
}

/// Renders the task rows in the requested format.
#[must_use]
pub fn render_tasks(rows: &[TaskRow], format: Format) -> String {
    match format {
        Format::Markdown => tasks_markdown(rows),
        Format::Csv => tasks_csv(rows),
    }
}

/// Renders the run summary rows in the requested format.
#[must_use]
pub fn render_runs(rows: &[RunRow], format: Format) -> String {
    match format {
        Format::Markdown => runs_markdown(rows),
        Format::Csv => runs_csv(rows),
    }
}

/// Markdown table of tasks, columns in [`TASK_COLUMNS`] order.
#[must_use]
pub fn tasks_markdown(rows: &[TaskRow]) -> String {
    let cells: Vec<Vec<String>> = rows.iter().map(task_cells).collect();
    markdown_table(&TASK_COLUMNS, &cells)
}

/// CSV of tasks with a header line, columns in [`TASK_COLUMNS`] order.
#[must_use]
pub fn tasks_csv(rows: &[TaskRow]) -> String {
    let cells: Vec<Vec<String>> = rows.iter().map(task_cells).collect();
    csv_document(&TASK_COLUMNS, &cells)
}

/// Markdown table of run totals, columns in [`RUN_COLUMNS`] order.
#[must_use]
pub fn runs_markdown(rows: &[RunRow]) -> String {
    let cells: Vec<Vec<String>> = rows.iter().map(run_cells).collect();
    markdown_table(&RUN_COLUMNS, &cells)
}

/// CSV of run totals with a header line, columns in [`RUN_COLUMNS`] order.
#[must_use]
pub fn runs_csv(rows: &[RunRow]) -> String {
    let cells: Vec<Vec<String>> = rows.iter().map(run_cells).collect();
    csv_document(&RUN_COLUMNS, &cells)
}

/// Quotes one CSV field when needed (RFC 4180), and neutralizes spreadsheet formulas.
///
/// A field that starts with `=`, `+`, `-`, `@`, tab, or CR is prefixed with `'`, so a
/// spreadsheet opens it as text instead of running it. Numeric columns are never negative
/// here, so this does not change their values.
#[must_use]
pub fn csv_escape(field: &str) -> String {
    let guarded = match field.chars().next() {
        Some('=' | '+' | '-' | '@' | '\t' | '\r') => format!("'{field}"),
        _ => field.to_owned(),
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

fn task_cells(row: &TaskRow) -> Vec<String> {
    vec![
        row.id.clone(),
        row.status.clone(),
        row.kind.clone(),
        row.parent.clone().unwrap_or_default(),
        row.depends_on.join(";"),
        row.labels.join(";"),
        row.assignee.clone().unwrap_or_default(),
        row.milestone.clone().unwrap_or_default(),
    ]
}

fn run_cells(row: &RunRow) -> Vec<String> {
    vec![
        row.provider.clone(),
        row.calls.to_string(),
        row.input_tokens.to_string(),
        row.output_tokens.to_string(),
        row.calls_without_token_counts.to_string(),
        row.wall_ms.to_string(),
    ]
}

fn csv_line(cells: &[String]) -> String {
    cells
        .iter()
        .map(|cell| csv_escape(cell))
        .collect::<Vec<_>>()
        .join(",")
}

fn csv_document(columns: &[&str], rows: &[Vec<String>]) -> String {
    let header: Vec<String> = columns.iter().map(|c| (*c).to_owned()).collect();
    let mut out = csv_line(&header);
    out.push_str("\r\n");
    for row in rows {
        out.push_str(&csv_line(row));
        out.push_str("\r\n");
    }
    out
}

/// One Markdown cell: pipes are escaped and line breaks become spaces, so a row stays one line.
fn md_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

fn markdown_table(columns: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = format!("| {} |\n", columns.join(" | "));
    out.push_str(&format!("|{}\n", " --- |".repeat(columns.len())));
    for row in rows {
        let cells: Vec<String> = row.iter().map(|cell| md_cell(cell)).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out
}

#[cfg(test)]
#[path = "export_test.rs"]
mod export_test;
