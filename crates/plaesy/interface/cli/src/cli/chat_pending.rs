//! Open questions for `plaesy chat`, kept as `decision` notes with status `proposed`.
//!
//! `/ask` writes a proposed note whose body lists the options. `/pending` lists the proposed
//! notes. `/pick` writes the chosen option into the same note and sets its status to `active`,
//! in place, through `plaesy memory update`. There is no second store.
//!
//! [`route`] is the policy gate for automatic choices: a choice may be taken without asking only
//! when it is reversible and the decision model's confidence reaches the threshold. Nothing calls
//! it yet, so every question waits for the owner.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::chat::decision_id;

pub const ASK_USAGE: &str = "usage: /ask <pertanyaan> | <opsi 1> | <opsi 2> [| ...]";
pub const PICK_USAGE: &str = "usage: /pick <id> <nomor opsi>";

/// The operations `/ask`, `/pending` and `/pick` need. [`MemoryPending`] is the real one; tests
/// use an in-memory store.
pub trait PendingStore {
    /// Queues a question and returns its id.
    fn ask(&self, question: &str, options: &[String]) -> Result<String, String>;
    /// Rendered list of open questions with numbered options.
    fn list(&self) -> Result<String, String>;
    /// Marks the question as answered with the option at 1-based `choice`.
    fn answer(&self, id: &str, choice: usize) -> Result<String, String>;
    /// `/decisions`: every visible decision with its status.
    fn list_decisions(&self) -> Result<String, String>;
    /// `/decisions update <id> <teks>`: rewrites the note in place with the new text.
    fn update(&self, id: &str, text: &str) -> Result<String, String>;
    /// `/decisions delete <id>`: archives the note. It stays in the vault and leaves the default list.
    fn archive(&self, id: &str) -> Result<String, String>;
    /// `/decisions` in a terminal: pick a decision, then change its text or archive it.
    /// Outside a terminal it is the plain list.
    fn decisions_menu(&self) -> Result<String, String> {
        self.list_decisions()
    }
    /// `/pending` in a terminal: pick a question and an option with the keyboard. Stores that
    /// cannot prompt fall back to the plain list.
    fn menu(&self) -> Result<String, String> {
        self.list()
    }
}

/// One proposed decision from `plaesy memory decisions --format json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Listed {
    pub id: String,
    pub title: String,
    pub path: String,
    #[serde(default)]
    pub status: String,
}

/// Reads the listing JSON. Anything else is an error.
pub fn parse_listing(raw: &str) -> Result<Vec<Listed>, String> {
    serde_json::from_str(raw).map_err(|err| format!("unreadable decision listing: {err}"))
}

/// The option lines of a proposed note body: `1. text`, `2. text`, in order.
pub fn parse_options(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let (number, text) = line.trim().split_once(". ")?;
            number.parse::<usize>().ok()?;
            Some(text.trim().to_owned())
        })
        .collect()
}

/// Body for a new proposed note: the numbered options, one per line.
pub fn body_with_options(options: &[String]) -> String {
    let mut body = String::from("Opsi:\n");
    for (index, option) in options.iter().enumerate() {
        body.push_str(&format!("{}. {option}\n", index + 1));
    }
    body
}

/// The note text after a choice: status becomes `active` and the chosen option is recorded.
/// Returns `None` if the note has no `status: "proposed"` line to change.
pub fn answered_text(content: &str, chosen: &str) -> Option<String> {
    if !content.contains("status: \"proposed\"") {
        return None;
    }
    let mut out = content.replacen("status: \"proposed\"", "status: \"active\"", 1);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("\nPilihan: {chosen}\n"));
    Some(out)
}

/// `<question> | <option> | <option>`: a question and at least two options.
pub fn parse_ask(rest: &str) -> Result<(String, Vec<String>), String> {
    let parts: Vec<&str> = rest.split(" | ").map(str::trim).collect();
    match parts.split_first() {
        Some((question, options))
            if !question.is_empty()
                && options.len() >= 2
                && options.iter().all(|o| !o.is_empty()) =>
        {
            Ok((
                (*question).to_owned(),
                options.iter().map(|o| (*o).to_owned()).collect(),
            ))
        }
        _ => Err(ASK_USAGE.to_owned()),
    }
}

/// `<id> <number>`, both positive integers. The id is the decision id from `/pending`.
pub fn parse_pick(rest: &str) -> Result<(String, usize), String> {
    let mut words = rest.split_whitespace();
    match (words.next(), words.next(), words.next()) {
        (Some(id), Some(choice), None) => match choice.parse::<usize>() {
            Ok(choice) if choice >= 1 => Ok((id.to_owned(), choice)),
            _ => Err(PICK_USAGE.to_owned()),
        },
        _ => Err(PICK_USAGE.to_owned()),
    }
}

/// `plaesy memory [--vault V] remember <question> <body> --folder decisions --type decision
/// --status proposed --id <id> [--project P]`.
pub fn ask_args(
    question: &str,
    options: &[String],
    id: &str,
    vault: Option<&Path>,
    project: Option<&str>,
) -> Vec<String> {
    let mut args = memory_prefix(vault);
    args.extend([
        "remember".to_owned(),
        question.to_owned(),
        body_with_options(options),
        "--folder".to_owned(),
        "decisions".to_owned(),
        "--type".to_owned(),
        "decision".to_owned(),
        "--status".to_owned(),
        "proposed".to_owned(),
        "--id".to_owned(),
        id.to_owned(),
    ]);
    if let Some(project) = project {
        args.push("--project".to_owned());
        args.push(project.to_owned());
    }
    args
}

/// `plaesy memory [--vault V] decisions --status proposed --format json [--project P]`.
pub fn list_args(vault: Option<&Path>, project: Option<&str>) -> Vec<String> {
    let mut args = memory_prefix(vault);
    args.extend([
        "decisions".to_owned(),
        "--status".to_owned(),
        "proposed".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]);
    if let Some(project) = project {
        args.push("--project".to_owned());
        args.push(project.to_owned());
    }
    args
}

/// `plaesy memory [--vault V] update --if-match <sha> --reason clarification --yes <path>`.
/// The new note text is sent on stdin.
pub fn update_args(sha: &str, path: &str, vault: Option<&Path>) -> Vec<String> {
    let mut args = memory_prefix(vault);
    args.extend([
        "update".to_owned(),
        "--if-match".to_owned(),
        sha.to_owned(),
        "--reason".to_owned(),
        "clarification".to_owned(),
        "--yes".to_owned(),
        path.to_owned(),
    ]);
    args
}

/// The note text after an in-place edit: the title line, the heading, and the statement line
/// change; everything else (id, status, supersedes, dates) is kept.
pub fn rewrite_decision(content: &str, text: &str) -> String {
    let quoted = serde_json::to_string(text).unwrap_or_else(|_| format!("\"{text}\""));
    let mut out = String::new();
    for line in content.lines() {
        let next = if line.starts_with("title: ") {
            format!("title: {quoted}")
        } else if line.starts_with("# ") {
            format!("# {text}")
        } else if line.starts_with("Keputusan: ") {
            format!("Keputusan: {text}")
        } else if line.starts_with("Pilihan: ") {
            format!("Pilihan: {text}")
        } else {
            line.to_owned()
        };
        out.push_str(&next);
        out.push('\n');
    }
    out
}

/// `plaesy memory [--vault V] decisions --format json [--project P]`: every decision the index
/// shows by default (proposed and active).
pub fn list_all_args(vault: Option<&Path>, project: Option<&str>) -> Vec<String> {
    let mut args = memory_prefix(vault);
    args.extend([
        "decisions".to_owned(),
        "--format".to_owned(),
        "json".to_owned(),
    ]);
    if let Some(project) = project {
        args.push("--project".to_owned());
        args.push(project.to_owned());
    }
    args
}

/// `plaesy memory [--vault V] archive --if-match <sha> --reason obsolete --yes <path>`.
pub fn archive_args(sha: &str, path: &str, vault: Option<&Path>) -> Vec<String> {
    let mut args = memory_prefix(vault);
    args.extend([
        "archive".to_owned(),
        "--if-match".to_owned(),
        sha.to_owned(),
        "--reason".to_owned(),
        "obsolete".to_owned(),
        "--yes".to_owned(),
        path.to_owned(),
    ]);
    args
}

fn memory_prefix(vault: Option<&Path>) -> Vec<String> {
    let mut args = vec!["memory".to_owned()];
    if let Some(vault) = vault {
        args.push("--vault".to_owned());
        args.push(vault.display().to_string());
    }
    args
}

/// Runs this same binary with `args`. Returns stdout on success, stderr on failure.
fn run_memory(args: &[String], stdin: Option<&str>) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let mut command = Command::new(exe);
    command.args(args);
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| err.to_string())?;
    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(text.as_bytes())
            .map_err(|err| err.to_string())?;
    }
    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

fn nanos_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
}

/// The real store: decision notes in the configured or given vault.
#[derive(Debug, Clone, Default)]
pub struct MemoryPending {
    /// Vault root. Required to read option text back, so `/pending` and `/pick` need it.
    pub vault: Option<PathBuf>,
    /// Project id that scopes the listing (`plaesy memory projects` shows ids).
    pub project: Option<String>,
    /// True when stdin is a terminal, so `/pending` may open the keyboard menu.
    pub interactive: bool,
}

impl MemoryPending {
    fn vault_root(&self) -> Result<&Path, String> {
        self.vault
            .as_deref()
            .ok_or_else(|| "pass --vault: option text is read from the vault".to_owned())
    }

    fn find(&self, id: &str) -> Result<Listed, String> {
        let raw = run_memory(
            &list_args(self.vault.as_deref(), self.project.as_deref()),
            None,
        )?;
        parse_listing(&raw)?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| format!("no pending question {id}"))
    }
}

impl PendingStore for MemoryPending {
    fn decisions_menu(&self) -> Result<String, String> {
        if !self.interactive {
            return self.list_decisions();
        }
        let root = self.vault_root()?;
        let items: Vec<Listed> = parse_listing(&run_memory(
            &list_all_args(self.vault.as_deref(), self.project.as_deref()),
            None,
        )?)?
        .into_iter()
        .filter(|item| root.join(&item.path).is_file())
        .collect();
        if items.is_empty() {
            return Ok("no decisions".to_owned());
        }
        let labels: Vec<String> = items
            .iter()
            .map(|item| format!("[{}] {}", item.status, item.title))
            .collect();
        let Some(index) = dialoguer::Select::new()
            .with_prompt(format!("{} keputusan", items.len()))
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(|err| err.to_string())?
        else {
            return Ok("cancelled".to_owned());
        };
        let item = &items[index];
        let actions = ["Ubah teks", "Arsipkan", "Batal"];
        let Some(action) = dialoguer::Select::new()
            .with_prompt(item.title.as_str())
            .items(&actions)
            .default(0)
            .interact_opt()
            .map_err(|err| err.to_string())?
        else {
            return Ok("cancelled".to_owned());
        };
        match action {
            0 => {
                let text: String = dialoguer::Input::new()
                    .with_prompt(item.title.as_str())
                    .with_initial_text(item.title.as_str())
                    .interact_text()
                    .map_err(|err| err.to_string())?;
                self.update(&item.id, text.trim())
            }
            1 => self.archive(&item.id),
            _ => Ok("cancelled".to_owned()),
        }
    }

    fn update(&self, id: &str, text: &str) -> Result<String, String> {
        let root = self.vault_root()?;
        let item = parse_listing(&run_memory(
            &list_all_args(self.vault.as_deref(), self.project.as_deref()),
            None,
        )?)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("no decision {id}"))?;
        let content = std::fs::read_to_string(root.join(&item.path))
            .map_err(|err| format!("{}: {err}", item.path))?;
        let updated = rewrite_decision(&content, text);
        let fingerprint = run_memory(
            &[
                memory_prefix(self.vault.as_deref()),
                vec!["fingerprint".to_owned(), item.path.clone()],
            ]
            .concat(),
            None,
        )?;
        let sha = fingerprint
            .split_whitespace()
            .next()
            .filter(|token| token.len() == 64)
            .ok_or_else(|| format!("no fingerprint for {}", item.path))?;
        run_memory(
            &update_args(sha, &item.path, self.vault.as_deref()),
            Some(&updated),
        )?;
        Ok(format!("{id} updated"))
    }

    fn list_decisions(&self) -> Result<String, String> {
        let root = self.vault_root()?;
        let items: Vec<Listed> = parse_listing(&run_memory(
            &list_all_args(self.vault.as_deref(), self.project.as_deref()),
            None,
        )?)?
        .into_iter()
        .filter(|item| root.join(&item.path).is_file())
        .collect();
        if items.is_empty() {
            return Ok("no decisions".to_owned());
        }
        Ok(items
            .iter()
            .map(|item| format!("{} [{}] {}", item.id, item.status, item.title))
            .collect::<Vec<_>>()
            .join(
                "
",
            ))
    }

    fn archive(&self, id: &str) -> Result<String, String> {
        let item = parse_listing(&run_memory(
            &list_all_args(self.vault.as_deref(), self.project.as_deref()),
            None,
        )?)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| format!("no decision {id}"))?;
        let fingerprint = run_memory(
            &[
                memory_prefix(self.vault.as_deref()),
                vec!["fingerprint".to_owned(), item.path.clone()],
            ]
            .concat(),
            None,
        )?;
        let sha = fingerprint
            .split_whitespace()
            .next()
            .filter(|token| token.len() == 64)
            .ok_or_else(|| format!("no fingerprint for {}", item.path))?;
        run_memory(&archive_args(sha, &item.path, self.vault.as_deref()), None)?;
        Ok(format!("{id} archived"))
    }

    fn menu(&self) -> Result<String, String> {
        if !self.interactive {
            return self.list();
        }
        let root = self.vault_root()?;
        let items = parse_listing(&run_memory(
            &list_args(self.vault.as_deref(), self.project.as_deref()),
            None,
        )?)?;
        // The index is shared across vaults, so skip notes this vault does not hold.
        let items: Vec<Listed> = items
            .into_iter()
            .filter(|item| root.join(&item.path).is_file())
            .collect();
        if items.is_empty() {
            return Ok("no pending questions".to_owned());
        }
        let item = if items.len() == 1 {
            &items[0]
        } else {
            let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
            let prompt = format!("{} pertanyaan terbuka", items.len());
            let Some(question) = dialoguer::Select::new()
                .with_prompt(prompt)
                .items(&titles)
                .default(0)
                .interact_opt()
                .map_err(|err| err.to_string())?
            else {
                return Ok("cancelled".to_owned());
            };
            &items[question]
        };
        let body = std::fs::read_to_string(root.join(&item.path))
            .map_err(|err| format!("{}: {err}", item.path))?;
        let options = parse_options(&body);
        let Some(choice) = dialoguer::Select::new()
            .with_prompt(item.title.as_str())
            .items(&options)
            .default(0)
            .interact_opt()
            .map_err(|err| err.to_string())?
        else {
            return Ok("cancelled".to_owned());
        };
        self.answer(&item.id, choice + 1)
    }

    fn ask(&self, question: &str, options: &[String]) -> Result<String, String> {
        let id = decision_id(question, nanos_now());
        let args = ask_args(
            question,
            options,
            &id,
            self.vault.as_deref(),
            self.project.as_deref(),
        );
        run_memory(&args, None)?;
        Ok(id)
    }

    fn list(&self) -> Result<String, String> {
        let root = self.vault_root()?;
        let items = parse_listing(&run_memory(
            &list_args(self.vault.as_deref(), self.project.as_deref()),
            None,
        )?)?;
        // The index is shared across vaults, so skip notes this vault does not hold.
        let items: Vec<Listed> = items
            .into_iter()
            .filter(|item| root.join(&item.path).is_file())
            .collect();
        if items.is_empty() {
            return Ok("no pending questions".to_owned());
        }
        let mut out = String::new();
        for item in items {
            let body = std::fs::read_to_string(root.join(&item.path))
                .map_err(|err| format!("{}: {err}", item.path))?;
            out.push_str(&format!("{} {}\n", item.id, item.title));
            for (index, option) in parse_options(&body).iter().enumerate() {
                out.push_str(&format!("   {}. {option}\n", index + 1));
            }
        }
        Ok(out.trim_end().to_owned())
    }

    fn answer(&self, id: &str, choice: usize) -> Result<String, String> {
        let root = self.vault_root()?;
        let item = self.find(id)?;
        let full = root.join(&item.path);
        let content =
            std::fs::read_to_string(&full).map_err(|err| format!("{}: {err}", item.path))?;
        let options = parse_options(&content);
        let chosen = choice
            .checked_sub(1)
            .and_then(|index| options.get(index))
            .ok_or_else(|| {
                format!(
                    "option {choice} does not exist for {id} (1..={})",
                    options.len()
                )
            })?;
        let updated = answered_text(&content, chosen)
            .ok_or_else(|| format!("{id} is not a proposed decision"))?;
        let fingerprint = run_memory(
            &[
                memory_prefix(self.vault.as_deref()),
                vec!["fingerprint".to_owned(), item.path.clone()],
            ]
            .concat(),
            None,
        )?;
        let sha = fingerprint
            .split_whitespace()
            .next()
            .filter(|token| token.len() == 64)
            .ok_or_else(|| format!("no fingerprint for {}", item.path))?;
        run_memory(
            &update_args(sha, &item.path, self.vault.as_deref()),
            Some(&updated),
        )?;
        Ok(format!("{id} answered: {chosen}"))
    }
}

/// What to do with a choice under the policy gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The choice may be taken without asking the owner.
    Take,
    /// Ask the owner: the choice is not reversible, or the model is not confident enough.
    Ask,
}

/// Automatic choices need both: reversibility, and a confidence at or above the threshold. A NaN
/// confidence fails the check, so a broken model never takes a choice.
pub fn route(reversible: bool, confidence: f64, threshold: f64) -> Route {
    if reversible && confidence >= threshold {
        Route::Take
    } else {
        Route::Ask
    }
}

#[cfg(test)]
#[path = "chat_pending_test.rs"]
mod chat_pending_test;
