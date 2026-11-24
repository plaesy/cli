//! `plaesy tasks board`: the tasks of every spec as text, one section per status column.

use std::path::Path;

use super::CliResult;
use crate::tasks::{self, Status, TaskRef, STATUSES};

/// Longest title printed in full. Longer titles are cut with `...`.
pub const TITLE_WIDTH: usize = 60;

/// Groups tasks into the status columns, in lifecycle order. Empty columns are kept so
/// every status shows up. Inside a column, more urgent priorities come first, then by name.
#[must_use]
pub fn columns(refs: &[TaskRef]) -> Vec<(Status, Vec<&TaskRef>)> {
    STATUSES
        .into_iter()
        .map(|status| {
            let mut column: Vec<&TaskRef> = refs.iter().filter(|r| r.status == status).collect();
            column.sort_by(|a, b| {
                tasks::priority_rank(&a.name)
                    .cmp(&tasks::priority_rank(&b.name))
                    .then_with(|| a.name.cmp(&b.name))
            });
            (status, column)
        })
        .collect()
}

/// Collapses whitespace runs to single spaces, then cuts the text to at most `max_chars`
/// characters. A cut text ends with `...`, and the cut never splits a character.
#[must_use]
pub fn truncate_title(text: &str, max_chars: usize) -> String {
    const ELLIPSIS: &str = "...";
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max_chars {
        return flat;
    }
    if max_chars <= ELLIPSIS.len() {
        return flat.chars().take(max_chars).collect();
    }
    let kept: String = flat.chars().take(max_chars - ELLIPSIS.len()).collect();
    format!("{}{ELLIPSIS}", kept.trim_end())
}

/// Renders the board as text: a header with the total, then one section per status with its
/// count and one line per task. A task shows its title (or its name when the title is empty),
/// cut to `title_width` characters, followed by its name.
#[must_use]
pub fn render(refs: &[TaskRef], title_width: usize) -> String {
    let mut out = format!("Task Board ({} tasks)\n", refs.len());
    for (status, column) in columns(refs) {
        out.push_str(&format!("\n{status} ({})\n", column.len()));
        if column.is_empty() {
            out.push_str("  (none)\n");
        }
        for task in column {
            let label = if task.title.trim().is_empty() {
                &task.name
            } else {
                &task.title
            };
            out.push_str(&format!(
                "  - {} ({})\n",
                truncate_title(label, title_width),
                task.name
            ));
        }
    }
    out
}

/// Prints the board for every spec under `tasks_dir`. A missing tasks folder prints empty columns.
pub fn run(tasks_dir: &Path) -> CliResult {
    let refs = tasks::list_all_tasks(tasks_dir)?;
    print!("{}", render(&refs, TITLE_WIDTH));
    Ok(())
}

#[cfg(test)]
#[path = "tasks_board_test.rs"]
mod tasks_board_test;
