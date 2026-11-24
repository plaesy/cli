//! `plaesy chat`: an interactive, multi-turn session with one configured provider.
//!
//! Provider calls are single-shot, so the transcript is rendered into each prompt. Switching
//! provider with `/provider <name>` keeps the transcript. Billable providers need `--confirm`.

use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::chat_pending;
use super::CliResult;
use crate::config::delegate::DelegateConfig;
use crate::delegate::providers::Getenv;
use crate::delegate::{self, flatten, Message, Provider, Request};
use sha2::{Digest, Sha256};

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Configured provider name; default is the first enabled, non-billable provider
    #[arg(long, default_value = "")]
    pub provider: String,
    /// Allow a billable provider to run
    #[arg(long)]
    pub confirm: bool,
    /// Timeout per call, for example 120s
    #[arg(long, default_value = "120s")]
    pub timeout: String,
    /// Maximum output tokens per reply; 0 means the provider default
    #[arg(long, default_value_t = 0)]
    pub max_tokens: u32,
    /// Repository whose .plaesy/config.json is read
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Vault where `/decision` notes are saved; default is the configured memory vault
    #[arg(long)]
    pub vault: Option<PathBuf>,
    /// Project id that scopes `/pending` and is set on `/ask` notes
    #[arg(long)]
    pub project: Option<String>,
}

/// A decision typed as `/decision <title> | <chosen> | <why>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub title: String,
    pub chosen: String,
    pub why: String,
}

const DECISION_USAGE: &str = "usage: /decision <judul> | <pilihan> | <alasan>";

/// Splits `<title> | <chosen> | <why>`. All three parts are required.
pub fn parse_decision(rest: &str) -> Result<Decision, String> {
    let parts: Vec<&str> = rest.split(" | ").map(str::trim).collect();
    match parts.as_slice() {
        [title, chosen, why] if ![title, chosen, why].iter().any(|p| p.is_empty()) => {
            Ok(Decision {
                title: (*title).to_owned(),
                chosen: (*chosen).to_owned(),
                why: (*why).to_owned(),
            })
        }
        _ => Err(DECISION_USAGE.to_owned()),
    }
}

/// Stable graph id for a decision. Built from the title and a time, so two decisions with the same
/// title get different ids. `--type decision` requires a non-empty id.
pub fn decision_id(title: &str, nanos: u128) -> String {
    let digest = Sha256::digest(format!("{title}\n{nanos}").as_bytes());
    let hex: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("decision-{hex}")
}

/// Arguments for `plaesy memory [--vault V] remember`. The note is a graph `decision` in the
/// `decisions` folder, so existing decision queries find it.
pub fn decision_args(decision: &Decision, vault: Option<&Path>, id: &str) -> Vec<String> {
    let mut args = vec!["memory".to_owned()];
    if let Some(vault) = vault {
        args.push("--vault".to_owned());
        args.push(vault.display().to_string());
    }
    let body = format!("Pilihan: {}\n\nAlasan: {}\n", decision.chosen, decision.why);
    args.extend([
        "remember".to_owned(),
        decision.title.clone(),
        body,
        "--folder".to_owned(),
        "decisions".to_owned(),
        "--type".to_owned(),
        "decision".to_owned(),
        "--status".to_owned(),
        "active".to_owned(),
        "--id".to_owned(),
        id.to_owned(),
    ]);
    args
}

/// Saves a decision by running this same binary's `memory remember`. Returns the command's
/// stdout on success, or its stderr on failure.
pub fn record_decision(decision: &Decision, vault: Option<&Path>) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let id = decision_id(&decision.title, nanos);
    let output = std::process::Command::new(exe)
        .args(decision_args(decision, vault, &id))
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// Saves a parsed decision and returns a short confirmation.
pub type DecisionRecorder<'a> = dyn Fn(&Decision) -> Result<String, String> + 'a;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// Turns of one conversation, oldest first.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    turns: Vec<(Role, String)>,
}

impl Transcript {
    pub fn push(&mut self, role: Role, text: impl Into<String>) {
        self.turns.push((role, text.into()));
    }

    pub fn clear(&mut self) {
        self.turns.clear();
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Every earlier turn, then the new user message, oldest first.
    pub fn messages(&self, next_user: &str) -> Vec<Message> {
        let mut out: Vec<Message> = self
            .turns
            .iter()
            .map(|(role, text)| match role {
                Role::User => Message::user(text.clone()),
                Role::Assistant => Message::assistant(text.clone()),
            })
            .collect();
        out.push(Message::user(next_user));
        out
    }

    /// The flattened prompt for the next user message, as a completion-style provider sees it.
    pub fn render(&self, next_user: &str) -> String {
        flatten(&self.messages(next_user))
    }
}

/// Opens a provider by its configured name.
pub type ProviderOpener<'a> = dyn Fn(&str) -> Result<Box<dyn Provider>, String> + 'a;

/// Per-call limits for the provider.
#[derive(Debug, Clone, Copy)]
pub struct CallLimits {
    pub timeout: Duration,
    pub max_tokens: u32,
}

/// One line typed at the prompt.
#[derive(Debug, PartialEq, Eq)]
pub enum Input {
    Empty,
    Exit,
    Clear,
    /// `/provider <name>`
    Provider(String),
    /// `/provider` with no name
    ProviderMissing,
    /// `/decision` followed by the raw `<title> | <chosen> | <why>` text
    Decision(String),
    /// `/decisions <teks>`: saves one statement as a decision, with no separate reason
    Decisions(String),
    /// `/ask` followed by the raw `<question> | <option> | ...` text
    Ask(String),
    /// `/pending`: list open questions
    Pending,
    /// `/pick` followed by the raw `<id> <number>` text
    Pick(String),
    Message(String),
}

pub fn parse_input(line: &str) -> Input {
    let text = line.trim();
    if text.is_empty() {
        return Input::Empty;
    }
    match text {
        "/exit" | "/quit" => return Input::Exit,
        "/clear" => return Input::Clear,
        "/pending" => return Input::Pending,
        _ => {}
    }
    for (command, make) in [
        ("/ask", Input::Ask as fn(String) -> Input),
        ("/pick", Input::Pick as fn(String) -> Input),
    ] {
        if let Some(rest) = text
            .strip_prefix(command)
            .filter(|rest| rest.is_empty() || rest.starts_with(' '))
        {
            return make(rest.trim().to_owned());
        }
    }
    if let Some(rest) = text
        .strip_prefix("/provider")
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    {
        let name = rest.trim();
        return if name.is_empty() {
            Input::ProviderMissing
        } else {
            Input::Provider(name.to_owned())
        };
    }
    if let Some(rest) = text
        .strip_prefix("/decision")
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    {
        return Input::Decision(rest.trim().to_owned());
    }
    if let Some(rest) = text
        .strip_prefix("/decisions")
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    {
        return Input::Decisions(rest.trim().to_owned());
    }
    Input::Message(text.to_owned())
}

/// Opens a provider by name. Gating (enabled, billable) is checked by [`delegate::select`].
pub fn open_provider(
    cfg: &DelegateConfig,
    name: &str,
    confirm: bool,
    getenv: &Getenv,
) -> Result<Box<dyn Provider>, String> {
    let entry = delegate::select(cfg, name, confirm)?;
    delegate::build(&entry, Arc::clone(getenv)).map_err(|err| err.to_string())
}

/// First enabled, non-billable provider in config order.
pub fn default_provider(cfg: &DelegateConfig) -> Result<String, String> {
    cfg.providers
        .iter()
        .find(|p| p.enabled && !p.billable)
        .map(|p| p.name.clone())
        .ok_or_else(|| {
            "no enabled, non-billable provider; pass --provider (see delegate.providers in .plaesy/config.json)"
                .to_owned()
        })
}

/// Runs the read-eval loop over `input`, writing replies and notices to `out`. A failed call is
/// reported and does not end the session or enter the transcript.
pub fn converse(
    mut provider: Box<dyn Provider>,
    open: &ProviderOpener<'_>,
    record: &DecisionRecorder<'_>,
    pending: &dyn chat_pending::PendingStore,
    limits: CallLimits,
    input: impl BufRead,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let mut transcript = Transcript::default();
    for line in input.lines() {
        let line = line?;
        match parse_input(&line) {
            Input::Empty => {}
            Input::Exit => break,
            Input::Clear => {
                transcript.clear();
                writeln!(out, "history cleared")?;
            }
            Input::ProviderMissing => writeln!(out, "usage: /provider <name>")?,
            Input::Provider(name) => match open(&name) {
                Ok(next) => {
                    provider = next;
                    writeln!(out, "provider: {name}")?;
                }
                Err(err) => writeln!(out, "error: {err}")?,
            },
            Input::Ask(rest) => match chat_pending::parse_ask(&rest) {
                Ok((question, options)) => match pending.ask(&question, &options) {
                    Ok(id) => writeln!(out, "proposed {id}: {question}")?,
                    Err(err) => writeln!(out, "error: {err}")?,
                },
                Err(usage) => writeln!(out, "{usage}")?,
            },
            Input::Pending => match pending.menu() {
                Ok(text) => writeln!(out, "{text}")?,
                Err(err) => writeln!(out, "error: {err}")?,
            },
            Input::Pick(rest) => match chat_pending::parse_pick(&rest) {
                Ok((id, choice)) => match pending.answer(&id, choice) {
                    Ok(text) => writeln!(out, "{text}")?,
                    Err(err) => writeln!(out, "error: {err}")?,
                },
                Err(usage) => writeln!(out, "{usage}")?,
            },
            Input::Decisions(text) => {
                if text.is_empty() {
                    match pending.decisions_menu() {
                        Ok(list) => writeln!(out, "{list}")?,
                        Err(err) => writeln!(out, "error: {err}")?,
                    }
                } else if let Some(rest) = text.strip_prefix("update ") {
                    match rest.trim().split_once(' ') {
                        Some((id, new_text)) if !new_text.trim().is_empty() => {
                            match pending.update(id, new_text.trim()) {
                                Ok(done) => writeln!(out, "{done}")?,
                                Err(err) => writeln!(out, "error: {err}")?,
                            }
                        }
                        _ => writeln!(out, "usage: /decisions update <id> <teks baru>")?,
                    }
                } else if text == "delete" {
                    writeln!(out, "usage: /decisions delete <id>")?;
                } else if let Some(id) = text.strip_prefix("delete ") {
                    match pending.archive(id.trim()) {
                        Ok(done) => writeln!(out, "{done}")?,
                        Err(err) => writeln!(out, "error: {err}")?,
                    }
                } else {
                    let decision = Decision {
                        title: text.clone(),
                        chosen: text,
                        why: String::new(),
                    };
                    match record(&decision) {
                        Ok(saved) => writeln!(out, "decision {}", saved.trim())?,
                        Err(err) => writeln!(out, "error: {err}")?,
                    }
                }
            }
            Input::Decision(rest) => match parse_decision(&rest) {
                Ok(decision) => match record(&decision) {
                    Ok(saved) => writeln!(out, "decision {}", saved.trim())?,
                    Err(err) => writeln!(out, "error: {err}")?,
                },
                Err(usage) => writeln!(out, "{usage}")?,
            },
            Input::Message(text) => {
                let request = Request {
                    prompt: String::new(),
                    max_output_tokens: limits.max_tokens,
                    timeout: limits.timeout,
                    dir: String::new(),
                };
                let messages = transcript.messages(&text);
                match delegate::run_messages(provider.as_ref(), &messages, &request) {
                    Ok(response) => {
                        writeln!(out, "{}", response.text)?;
                        transcript.push(Role::User, text);
                        transcript.push(Role::Assistant, response.text);
                    }
                    Err(err) => writeln!(out, "error: {err}")?,
                }
            }
        }
        out.flush()?;
    }
    Ok(())
}

pub fn run(args: &Args) -> CliResult {
    let cfg = super::delegate::load_delegate_config(&args.path)?;
    let timeout = crate::reach::duration::parse(&args.timeout)
        .ok_or_else(|| format!("invalid --timeout {:?}", args.timeout))?;
    let getenv: Getenv = Arc::new(|name: &str| std::env::var(name).ok());
    let name = if args.provider.is_empty() {
        default_provider(&cfg)?
    } else {
        args.provider.clone()
    };
    let provider = open_provider(&cfg, &name, args.confirm, &getenv)?;
    let open = |next: &str| open_provider(&cfg, next, args.confirm, &getenv);

    let vault = args.vault.clone();
    let record = move |decision: &Decision| record_decision(decision, vault.as_deref());

    println!(
        "chat with {name}. /provider <name> switches, /decision <judul> | <pilihan> | <alasan> saves a decision, /clear resets, /exit quits."
    );
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    converse(
        provider,
        &open,
        &record,
        &chat_pending::MemoryPending {
            vault: args.vault.clone(),
            project: args.project.clone(),
            interactive: std::io::IsTerminal::is_terminal(&io::stdin()),
        },
        CallLimits {
            timeout,
            max_tokens: args.max_tokens,
        },
        stdin.lock(),
        &mut stdout,
    )
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;
