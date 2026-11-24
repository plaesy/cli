//! `/tasks` in `plaesy chat`: the task folders under `.plaesy/tasks/<status>/<id>/`, shown as a
//! keyboard menu in a terminal, and changed through the same `plaesy tasks` commands a person
//! would type. The menu only offers actions the task's status allows, so an illegal move cannot
//! be picked.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::chat_text::{self, Lang};

/// Status folders in lifecycle order.
pub const STATUSES: &[&str] = &["backlog", "todo", "doing", "review", "blocked", "done"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    pub id: String,
    pub status: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Backlog to todo.
    Next,
    /// Todo to doing.
    Start,
    /// Doing to blocked, with a reason.
    Block,
    /// Blocked back to todo.
    Unblock,
    /// Review to done (reviewer decision).
    Complete,
    /// Print the task's raw content.
    Show,
}

/// What a status allows. `Show` is always allowed.
pub fn actions_for(status: &str) -> Vec<Action> {
    let mut out = match status {
        "backlog" => vec![Action::Next],
        "todo" => vec![Action::Start],
        "doing" => vec![Action::Block],
        "blocked" => vec![Action::Unblock],
        "review" => vec![Action::Complete],
        _ => Vec::new(),
    };
    out.push(Action::Show);
    out
}

/// The status a task moves to, or `None` for an action that only shows it.
pub fn target_status(action: Action) -> Option<&'static str> {
    match action {
        Action::Next | Action::Unblock => Some("todo"),
        Action::Start => Some("doing"),
        Action::Block => Some("blocked"),
        Action::Complete => Some("done"),
        Action::Show => None,
    }
}

/// The `plaesy tasks` arguments for an action. `reason` is used only by `Block`.
pub fn command_args(action: Action, id: &str, reason: &str) -> Vec<String> {
    let mut args = vec!["tasks".to_owned()];
    match action {
        Action::Next => args.extend(["next".to_owned(), id.to_owned()]),
        Action::Start => args.extend(["start".to_owned(), id.to_owned()]),
        Action::Block => args.extend(["block".to_owned(), id.to_owned(), reason.to_owned()]),
        Action::Unblock => args.extend(["unblock".to_owned(), id.to_owned()]),
        Action::Complete => args.extend(["complete".to_owned(), id.to_owned()]),
        Action::Show => args.extend(["show".to_owned(), id.to_owned()]),
    }
    args
}

pub fn action_label(lang: Lang, action: Action) -> &'static str {
    match action {
        Action::Next => chat_text::action_next(lang),
        Action::Start => chat_text::action_start(lang),
        Action::Block => chat_text::action_block(lang),
        Action::Unblock => chat_text::action_unblock(lang),
        Action::Complete => chat_text::action_complete(lang),
        Action::Show => chat_text::action_show(lang),
    }
}

/// The title is the first `# ` heading of `task.md`, or the id when there is none.
pub fn title_from(content: &str, fallback: &str) -> String {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(|title| title.trim().to_owned())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

/// Every task under `repo/.plaesy/tasks`, in lifecycle order then by id. A missing folder is an
/// empty list, not an error.
pub fn scan(repo: &Path) -> Vec<TaskItem> {
    let root = repo.join(".plaesy").join("tasks");
    let mut out = Vec::new();
    for status in STATUSES {
        let Ok(entries) = fs::read_dir(root.join(status)) else {
            continue;
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        for id in ids {
            let content =
                fs::read_to_string(root.join(status).join(&id).join("task.md")).unwrap_or_default();
            let title = title_from(&content, &id);
            out.push(TaskItem {
                id,
                status: (*status).to_owned(),
                title,
            });
        }
    }
    out
}

/// Plain list for a non-terminal session: one line per task.
pub fn render(lang: Lang, items: &[TaskItem]) -> String {
    if items.is_empty() {
        return chat_text::no_tasks(lang).to_owned();
    }
    items
        .iter()
        .map(|item| format!("[{}] {} {}", item.status, item.id, item.title))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Runs `plaesy tasks <action>` in the repository. Returns stdout on success, stderr on failure.
pub fn run_command(repo: &Path, args: &[String]) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let output = Command::new(exe)
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// A status change made from the menu: the task, where it was, and where it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub id: String,
    pub from: String,
    pub to: String,
}

/// `/tasks` in a terminal: pick a task, then the actions its status allows. Outside a terminal
/// the caller uses [`render`] instead. The change is returned only when the action moved the
/// task, so the caller can fire `task.changed`.
pub fn menu(repo: &Path, lang: Lang) -> Result<(String, Option<Change>), String> {
    let items = scan(repo);
    if items.is_empty() {
        return Ok((chat_text::no_tasks(lang).to_owned(), None));
    }
    let labels: Vec<String> = items
        .iter()
        .map(|item| format!("[{}] {} {}", item.status, item.id, item.title))
        .collect();
    let Some(index) = dialoguer::Select::new()
        .with_prompt(chat_text::tasks_count(lang, items.len()))
        .items(&labels)
        .default(0)
        .interact_opt()
        .map_err(|err| err.to_string())?
    else {
        return Ok((chat_text::cancelled(lang).to_owned(), None));
    };
    let item = &items[index];
    let actions = actions_for(&item.status);
    let action_labels: Vec<&str> = actions.iter().map(|a| action_label(lang, *a)).collect();
    let Some(choice) = dialoguer::Select::new()
        .with_prompt(item.title.as_str())
        .items(&action_labels)
        .default(0)
        .interact_opt()
        .map_err(|err| err.to_string())?
    else {
        return Ok((chat_text::cancelled(lang).to_owned(), None));
    };
    let action = actions[choice];
    let reason = if action == Action::Block {
        dialoguer::Input::<String>::new()
            .with_prompt(chat_text::block_reason_prompt(lang))
            .interact_text()
            .map_err(|err| err.to_string())?
    } else {
        String::new()
    };
    let output = run_command(repo, &command_args(action, &item.id, &reason))?;
    let text = if output.is_empty() {
        chat_text::task_done(lang, &item.id)
    } else {
        output
    };
    let change = target_status(action).map(|to| Change {
        id: item.id.clone(),
        from: item.status.clone(),
        to: to.to_owned(),
    });
    Ok((text, change))
}

/// The repository's task root, for callers that need the path.
pub fn tasks_root(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("tasks")
}

#[cfg(test)]
#[path = "chat_tasks_test.rs"]
mod chat_tasks_test;
