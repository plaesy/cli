//! `plaesy tasks board`: every spec's tasks as text columns, one per status folder.

use std::path::Path;

use super::CliResult;
use crate::tasks::{self, Status, TaskRef, STATUSES};

/// Longest title shown on a card, in characters. Longer titles end with `…`.
pub const MAX_TITLE_CHARS: usize = 60;

/// Each status with its tasks, in lifecycle order. Every status appears, even when empty.
/// Inside a column, tasks are ordered by priority prefix; ties keep their name order.
pub fn group_by_status(refs: &[TaskRef]) -> Vec<(Status, Vec<&TaskRef>)> {
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

/// Shortens `text` to at most `max_chars` characters, counting `char`s rather than bytes.
/// A shortened text ends with `…` and still fits in `max_chars`.
pub fn truncate_title(text: &str, max_chars: usize) -> String {
    // Collapse whitespace runs so a title never breaks the one-line card.
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max_chars {
        return flat;
    }
    if max_chars == 0 {
        return String::new();
    }
    let kept: String = flat.chars().take(max_chars - 1).collect();
    // Drop the space left before the ellipsis so the cut never reads "gamma …".
    format!("{}…", kept.trim_end())
}

/// The text a card shows before truncation: its title, or its name when the title is empty.
fn card_title(task: &TaskRef) -> &str {
    if task.title.trim().is_empty() {
        &task.name
    } else {
        &task.title
    }
}

/// The board as lines of text: a header, then one section per status column.
pub fn render_board(refs: &[TaskRef]) -> Vec<String> {
    let mut lines = vec![
        format!("📋 Task Board ({} tasks)", refs.len()),
        String::new(),
    ];
    for (status, column) in group_by_status(refs) {
        lines.push(format!("{status} ({})", column.len()));
        if column.is_empty() {
            lines.push("  (none)".to_owned());
        }
        for task in column {
            lines.push(format!(
                "  - {}",
                truncate_title(card_title(task), MAX_TITLE_CHARS)
            ));
            lines.push(format!("    {}", task.name));
        }
        lines.push(String::new());
    }
    lines
}

/// Prints the board for every task under `tasks_dir`. A missing folder prints empty columns.
///
/// # Errors
///
/// Filesystem or symlink problems reported by the task listing.
pub fn run(tasks_dir: &Path) -> CliResult {
    let refs = tasks::list_all_tasks(tasks_dir)?;
    for line in render_board(&refs) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "tasks_board_test.rs"]
mod tasks_board_test;
