//! `plaesy tasks comment`, `plaesy tasks comments`: comments on a task and their @mentions.

use std::path::Path;

use super::CliResult;
use crate::tasks::comments;

/// Adds a comment to `task`. `@name` mentions are stored with the comment.
pub fn add(task: &str, text: &str, tasks_dir: &Path) -> CliResult {
    let comment = comments::add_comment(tasks_dir, task, text)?;
    println!("[✓] Comment {} added to {task}", comment.id);
    if !comment.mentions.is_empty() {
        println!("    mentions: {}", format_mentions(&comment.mentions));
    }
    Ok(())
}

/// Lists the comments of `task` in the order they were written.
pub fn list(task: &str, tasks_dir: &Path) -> CliResult {
    let items = comments::list_comments(tasks_dir, task)?;
    if items.is_empty() {
        println!("[!] No comments on {task}");
        return Ok(());
    }
    for comment in &items {
        println!("#{} {}", comment.id, comment.created_at);
        for line in comment.body.lines() {
            println!("  {line}");
        }
        if !comment.mentions.is_empty() {
            println!("  mentions: {}", format_mentions(&comment.mentions));
        }
    }
    println!("{} comment(s)", items.len());
    Ok(())
}

/// Removes the comment with `id` from `task`.
pub fn delete(task: &str, id: u64, tasks_dir: &Path) -> CliResult {
    comments::remove_comment(tasks_dir, task, id)?;
    println!("[✓] Comment {id} removed from {task}");
    Ok(())
}

fn format_mentions(names: &[String]) -> String {
    names
        .iter()
        .map(|name| format!("@{name}"))
        .collect::<Vec<_>>()
        .join(", ")
}
