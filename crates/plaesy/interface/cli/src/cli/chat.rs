//! `plaesy chat`: an interactive, multi-turn session with one configured provider.
//!
//! Provider calls are single-shot, so the transcript is rendered into each prompt. Switching
//! provider with `/provider <name>` keeps the transcript. Billable providers need `--confirm`.

use std::collections::VecDeque;
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use super::chat_agents;
use super::chat_hooks;
use super::chat_lsp;
use super::chat_mcp;
use super::chat_pending;
use super::chat_plugins;
use super::chat_roles;
use super::chat_tasks;
use super::chat_text::{self, Lang, Usage};
use super::chat_tui;
use super::CliResult;
use crate::config::delegate::DelegateConfig;
use crate::delegate::providers::Getenv;
use crate::delegate::{self, flatten, Message, Provider, Request};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

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
    /// This session's name on the relay. Overrides agent.name in settings.json, so two sessions can differ
    #[arg(long)]
    pub agent: Option<String>,
    /// Full-screen terminal view instead of the line prompt
    #[arg(long)]
    pub tui: bool,
    /// Role to start with: a name from `roles` in settings.json. Default: "default"
    #[arg(long)]
    pub role: Option<String>,
    /// Repository whose .plaesy/settings.json is read
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Vault where `/decision` notes are saved; default is the configured memory vault
    #[arg(long)]
    pub vault: Option<PathBuf>,
    /// Language for chat text: en or id. Default: "language" in ~/.plaesy/settings.json, then en
    #[arg(long)]
    pub lang: Option<String>,
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

/// Splits `<title> | <chosen> | <why>`. All three parts are required.
pub fn parse_decision(rest: &str) -> Result<Decision, Usage> {
    let parts: Vec<&str> = rest.split(" | ").map(str::trim).collect();
    match parts.as_slice() {
        [title, chosen, why] if ![title, chosen, why].iter().any(|p| p.is_empty()) => {
            Ok(Decision {
                title: (*title).to_owned(),
                chosen: (*chosen).to_owned(),
                why: (*why).to_owned(),
            })
        }
        _ => Err(Usage::Decision),
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
    let body = if decision.why.is_empty() {
        format!("statement: {}\n", decision.chosen)
    } else {
        format!("chosen: {}\n\nwhy: {}\n", decision.chosen, decision.why)
    };
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

    /// Every earlier turn as messages, oldest first, without a new user message.
    pub fn history(&self) -> Vec<Message> {
        self.turns
            .iter()
            .map(|(role, text)| match role {
                Role::User => Message::user(text.clone()),
                Role::Assistant => Message::assistant(text.clone()),
            })
            .collect()
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
#[derive(Debug, Clone)]
pub struct ChatSettings {
    pub timeout: Duration,
    pub max_tokens: u32,
    pub lang: Lang,
    /// Repository whose `.plaesy/tasks` folder `/tasks` shows.
    pub repo: PathBuf,
    /// True when stdin is a terminal, so `/tasks` and `/pending` may open a menu.
    pub interactive: bool,
    /// Hooks from the repository and the user, in configuration order.
    pub hooks: Vec<chat_hooks::HookSpec>,
    /// Fingerprints of repository hooks the owner approved.
    pub approved: Vec<String>,
    /// MCP servers from the repository and the user.
    pub mcp_servers: Vec<chat_mcp::McpServer>,
    /// Roles from the repository and the user.
    pub roles: Vec<chat_roles::Role>,
    /// The role to start with.
    pub role: String,
    /// Skill store, where roles find their skills.
    pub skills_dir: PathBuf,
    /// Plugins from the user folder and the repository.
    pub plugins: Vec<chat_plugins::Plugin>,
    /// This chat's name on the relay (`agent.name` in settings.json; default "chat").
    pub agent_name: String,
    /// Peer agents and sessions offered by `@` (`agent.peers` in settings.json).
    pub peers: Vec<String>,
    /// Provider `/advisor` asks (`chat.advisor` in settings.json).
    pub advisor: Option<String>,
    /// Providers `/council` asks, in configuration order.
    pub council: Vec<String>,
    /// Language servers for `/lsp` (`lsp.servers` in settings.json).
    pub lsp_servers: Vec<chat_lsp::Server>,
    /// Every provider registered in settings, in configuration order (`/providers`).
    pub providers: Vec<ProviderEntry>,
    /// Write a turn-end mark (a 0x1E byte) after each line is handled. The full-screen view uses it
    /// to show that a reply is still coming. The plain prompt leaves it off.
    pub turn_marks: bool,
}

/// Written after each handled line when `turn_marks` is on.
pub const TURN_MARK: u8 = 0x1e;

/// One registered provider, as listed by `/providers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEntry {
    pub name: String,
    pub enabled: bool,
    pub billable: bool,
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
    /// `/tasks`: the task folders, as a menu in a terminal
    Tasks,
    /// `/mcp` lists servers; `/mcp call <server> <tool> <json>` calls an allowed tool
    Mcp(String),
    /// `/role` lists roles; `/role <name>` switches to one
    Role(String),
    /// `/plugins` lists plugins and their commands
    Plugins,
    /// `/agent inbox|reply|ack` for messages to other agents, through the relay
    Agent(String),
    /// `@name <text>`: send a message to another agent or chat session, through the relay
    Mention(String, String),
    /// A bare `@`: pick a peer from the list, then type the message
    MentionMenu,
    /// `/plugin run <plugin> <command> [args...]`
    Plugin(String),
    /// `/pick` followed by the raw `<id> <number>` text
    Pick(String),
    /// `/goal` shows the session goal; `/goal <text>` sets it; `/goal clear` removes it
    Goal(String),
    /// `/compact` replaces the transcript with one summary written by the provider
    Compact,
    /// `/providers`: every registered provider, with the one in use marked
    Providers,
    /// `/debug`: what this session is running with, without secrets or transcript text
    Debug,
    /// `/lsp <definition|references|hover> <file>:<line>:<col>`, read-only
    Lsp(String),
    /// `/advisor <question>`: the advisor provider reviews the conversation and the question
    Advisor(String),
    /// `/council <question>`: every enabled provider answers the question on its own
    Council(String),
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
        "/tasks" => return Input::Tasks,
        "/plugins" => return Input::Plugins,
        "/compact" => return Input::Compact,
        "/providers" => return Input::Providers,
        "/debug" => return Input::Debug,
        "/goal" => return Input::Goal(String::new()),
        _ => {}
    }
    if let Some(rest) = text.strip_prefix("/goal ") {
        return Input::Goal(rest.trim().to_owned());
    }
    if let Some(rest) = text.strip_prefix("/lsp ") {
        return Input::Lsp(rest.trim().to_owned());
    }
    if text == "/advisor" {
        return Input::Advisor(String::new());
    }
    if let Some(rest) = text.strip_prefix("/advisor ") {
        return Input::Advisor(rest.trim().to_owned());
    }
    if text == "/council" {
        return Input::Council(String::new());
    }
    if let Some(rest) = text.strip_prefix("/council ") {
        return Input::Council(rest.trim().to_owned());
    }
    for (command, make) in [
        ("/ask", Input::Ask as fn(String) -> Input),
        ("/pick", Input::Pick as fn(String) -> Input),
        ("/mcp", Input::Mcp as fn(String) -> Input),
        ("/role", Input::Role as fn(String) -> Input),
        ("/plugin", Input::Plugin as fn(String) -> Input),
        ("/agent", Input::Agent as fn(String) -> Input),
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
    if text == "@" {
        return Input::MentionMenu;
    }
    if let Some(rest) = text.strip_prefix('@') {
        let (name, body) = match rest.split_once(' ') {
            Some((name, body)) => (name, body.trim()),
            None => (rest, ""),
        };
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Input::Mention(name.to_owned(), body.to_owned());
        }
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
            "no enabled, non-billable provider; pass --provider (see delegate.providers in .plaesy/settings.json)"
                .to_owned()
        })
}

/// Splits `"<word> <rest>"`; both parts must be non-empty.
fn split_word(text: &str) -> Option<(&str, &str)> {
    let (head, tail) = text.trim().split_once(' ')?;
    let tail = tail.trim();
    (!head.is_empty() && !tail.is_empty()).then_some((head, tail))
}

/// `/agent`: messages to other agents through the relay. Incoming text is shown, never acted on.
fn agent_command(repo: &Path, from: &str, rest: &str) -> Result<String, String> {
    let usage = || chat_text::usage(Lang::En, Usage::Agent).to_owned();
    let inbox = || -> Result<Vec<chat_agents::Incoming>, String> {
        chat_agents::parse_inbox(&chat_agents::run_relay(
            repo,
            &chat_agents::inbox_args(from),
        )?)
    };
    let rest = rest.trim();
    if rest == "inbox" {
        return Ok(chat_agents::render_incoming(&inbox()?));
    }
    if let Some(ack) = rest.strip_prefix("ack ") {
        let id = ack.trim();
        chat_agents::run_relay(repo, &chat_agents::ack_args(id, from))?;
        return Ok(format!("acknowledged {id}"));
    }
    if let Some(send) = rest.strip_prefix("send ") {
        let (to, text) = split_word(send).ok_or_else(usage)?;
        chat_agents::check_outgoing(text, 1)?;
        let id = chat_agents::new_id(from, to, text, chat_agents::nanos_now());
        chat_agents::run_relay(
            repo,
            &chat_agents::send_args(from, to, &id, &chat_agents::encode(1, text)),
        )?;
        return Ok(format!("sent {id} to {to}"));
    }
    if let Some(reply) = rest.strip_prefix("reply ") {
        let (id, text) = split_word(reply).ok_or_else(usage)?;
        let incoming = inbox()?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| format!("no message {id}"))?;
        let depth = chat_agents::decode(&incoming.body).depth + 1;
        chat_agents::check_outgoing(text, depth)?;
        let new_id = chat_agents::new_id(from, &incoming.sender, text, chat_agents::nanos_now());
        chat_agents::run_relay(
            repo,
            &chat_agents::send_args(
                from,
                &incoming.sender,
                &new_id,
                &chat_agents::encode(depth, text),
            ),
        )?;
        return Ok(format!("replied {new_id} to {}", incoming.sender));
    }
    Err(usage())
}

/// Runs the hooks of one event, prints their warnings, and returns the reason when a hook blocked.
fn fire_and_print(
    out: &mut impl Write,
    settings: &ChatSettings,
    lang: Lang,
    event: &str,
    pairs: &[(&str, &str)],
) -> io::Result<Option<String>> {
    let data: BTreeMap<String, String> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    let outcome = chat_hooks::fire(&settings.hooks, &settings.approved, event, &data);
    for warning in &outcome.warnings {
        writeln!(out, "{}", chat_text::warning(lang, warning))?;
    }
    Ok(outcome.blocked)
}

/// Takes the next line to run. A steered line (one starting with `!`) goes first, and its `!` is
/// removed; otherwise the oldest line runs. The caller checks that the queue is not empty.
pub fn take_next(queue: &mut VecDeque<String>) -> String {
    match queue
        .iter()
        .position(|line| line.trim_start().starts_with('!'))
    {
        Some(index) => queue
            .remove(index)
            .unwrap_or_default()
            .trim_start()
            .trim_start_matches('!')
            .to_owned(),
        None => queue.pop_front().unwrap_or_default(),
    }
}

/// Runs the read-eval loop over `input`, writing replies and notices to `out`. A failed call is
/// reported and does not end the session or enter the transcript.
pub fn converse(
    mut provider: Box<dyn Provider>,
    open: &ProviderOpener<'_>,
    record: &DecisionRecorder<'_>,
    pending: &dyn chat_pending::PendingStore,
    settings: ChatSettings,
    input: impl BufRead + Send + 'static,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let lang = settings.lang;
    let mut active =
        chat_roles::find(&settings.roles, &settings.role).unwrap_or_else(|| chat_roles::Role {
            name: chat_roles::DEFAULT_ROLE.to_owned(),
            ..chat_roles::Role::default()
        });
    for warning in chat_roles::system_text(&active, &settings.skills_dir).1 {
        writeln!(out, "{}", chat_text::warning(lang, &warning))?;
    }
    fire_and_print(out, &settings, lang, "session.start", &[])?;
    let mut transcript = Transcript::default();
    // The session goal is kept for this run only; it is never written to disk.
    let mut goal: Option<String> = None;
    // A reader thread forwards lines as they arrive, so lines typed while a reply is running wait
    // in the queue. A line starting with `!` is steered: it runs next, ahead of the other queued lines.
    let (sender, receiver) = mpsc::channel::<io::Result<String>>();
    thread::spawn(move || {
        for line in input.lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let mut queue: VecDeque<String> = VecDeque::new();
    loop {
        if queue.is_empty() {
            match receiver.recv() {
                Ok(line) => queue.push_back(line?),
                Err(_) => break,
            }
        }
        while let Ok(line) = receiver.try_recv() {
            queue.push_back(line?);
        }
        let waiting = queue.len() - 1;
        let line = take_next(&mut queue);
        if waiting > 0 {
            writeln!(out, "{}", chat_text::queued(lang, waiting))?;
        }
        match parse_input(&line) {
            Input::Empty => {}
            Input::Exit => break,
            Input::Clear => {
                transcript.clear();
                writeln!(out, "{}", chat_text::history_cleared(lang))?;
            }
            Input::ProviderMissing => writeln!(out, "{}", chat_text::usage(lang, Usage::Provider))?,
            Input::Provider(name) => match open(&name) {
                Ok(next) => {
                    provider = next;
                    writeln!(out, "{}", chat_text::provider_switched(lang, &name))?;
                }
                Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
            },
            Input::Ask(rest) => match chat_pending::parse_ask(&rest) {
                Ok((question, options)) => match pending.ask(&question, &options) {
                    Ok(id) => writeln!(
                        out,
                        "{}",
                        chat_text::question_proposed(lang, &id, &question)
                    )?,
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                },
                Err(kind) => writeln!(out, "{}", chat_text::usage(lang, kind))?,
            },
            Input::Mcp(rest) => {
                let result = if rest.is_empty() {
                    Ok(chat_mcp::list_all(
                        &settings.mcp_servers,
                        &settings.approved,
                    ))
                } else if let Some(call_args) = rest.strip_prefix("call ") {
                    let mut parts = call_args.trim().splitn(3, ' ');
                    match (parts.next(), parts.next()) {
                        (Some(server), Some(tool)) if !server.is_empty() && !tool.is_empty() => {
                            let json = parts.next().unwrap_or("{}");
                            match serde_json::from_str::<serde_json::Value>(json) {
                                Ok(_) if !chat_roles::tool_allowed(&active, tool) => {
                                    Err(format!("role {} does not allow tool {tool}", active.name))
                                }
                                Ok(arguments) => chat_mcp::call(
                                    &settings.mcp_servers,
                                    &settings.approved,
                                    server,
                                    tool,
                                    arguments,
                                ),
                                Err(err) => Err(format!("arguments must be JSON: {err}")),
                            }
                        }
                        _ => Err(chat_text::usage(lang, Usage::Mcp).to_owned()),
                    }
                } else {
                    Err(chat_text::usage(lang, Usage::Mcp).to_owned())
                };
                match result {
                    Ok(text) => writeln!(out, "{text}")?,
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err))?,
                }
            }
            Input::Role(name) => {
                if name.is_empty() {
                    let mut names: Vec<String> =
                        settings.roles.iter().map(|r| r.name.clone()).collect();
                    if !names.iter().any(|n| n == chat_roles::DEFAULT_ROLE) {
                        names.insert(0, chat_roles::DEFAULT_ROLE.to_owned());
                    }
                    let listing: Vec<String> = names
                        .iter()
                        .map(|n| {
                            if *n == active.name {
                                format!("* {n}")
                            } else {
                                format!("  {n}")
                            }
                        })
                        .collect();
                    writeln!(
                        out,
                        "{}",
                        listing.join(
                            "
"
                        )
                    )?;
                } else {
                    match chat_roles::find(&settings.roles, &name) {
                        None => writeln!(
                            out,
                            "{}",
                            chat_text::error(lang, &format!("unknown role {name}"))
                        )?,
                        Some(next) => {
                            if let Some(provider_name) = next.provider.clone() {
                                match open(&provider_name) {
                                    Ok(new_provider) => provider = new_provider,
                                    Err(err) => {
                                        writeln!(out, "{}", chat_text::error(lang, &err))?;
                                        continue;
                                    }
                                }
                            }
                            for warning in chat_roles::system_text(&next, &settings.skills_dir).1 {
                                writeln!(out, "{}", chat_text::warning(lang, &warning))?;
                            }
                            writeln!(out, "role: {}", next.name)?;
                            active = next;
                        }
                    }
                }
            }
            Input::Agent(rest) => {
                match agent_command(&settings.repo, &settings.agent_name, &rest) {
                    Ok(text) => writeln!(out, "{text}")?,
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err))?,
                }
            }
            Input::Mention(to, body) => match agent_command(
                &settings.repo,
                &settings.agent_name,
                &format!("send {to} {body}"),
            ) {
                Ok(text) => writeln!(out, "{text}")?,
                Err(err) => writeln!(out, "{}", chat_text::error(lang, &err))?,
            },
            Input::MentionMenu => {
                if settings.peers.is_empty() {
                    writeln!(
                        out,
                        "{}",
                        chat_text::error(lang, "no peers; add agent.peers to settings.json")
                    )?;
                } else if !settings.interactive {
                    let listing: Vec<String> =
                        settings.peers.iter().map(|p| format!("@{p}")).collect();
                    writeln!(out, "{}", listing.join(", "))?;
                } else {
                    let labels: Vec<String> =
                        settings.peers.iter().map(|p| format!("@{p}")).collect();
                    match dialoguer::Select::new()
                        .with_prompt("@")
                        .items(&labels)
                        .default(0)
                        .interact_opt()
                    {
                        Ok(Some(index)) => {
                            let name = settings.peers[index].clone();
                            let prompt = format!("@{name}");
                            match dialoguer::Input::<String>::new()
                                .with_prompt(prompt)
                                .interact_text()
                            {
                                Ok(body) if !body.trim().is_empty() => {
                                    match agent_command(
                                        &settings.repo,
                                        &settings.agent_name,
                                        &format!("send {name} {}", body.trim()),
                                    ) {
                                        Ok(text) => writeln!(out, "{text}")?,
                                        Err(err) => {
                                            writeln!(out, "{}", chat_text::error(lang, &err))?
                                        }
                                    }
                                }
                                _ => writeln!(out, "{}", chat_text::cancelled(lang))?,
                            }
                        }
                        _ => writeln!(out, "{}", chat_text::cancelled(lang))?,
                    }
                }
            }
            Input::Plugins => writeln!(
                out,
                "{}",
                chat_plugins::list(&settings.plugins, &settings.approved)
            )?,
            Input::Plugin(rest) => {
                let words: Vec<&str> = rest.split_whitespace().collect();
                let result = match words.as_slice() {
                    ["run", plugin, command, args @ ..] => {
                        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
                        chat_plugins::run(
                            &settings.plugins,
                            &settings.approved,
                            plugin,
                            command,
                            &args,
                            settings.timeout,
                        )
                    }
                    _ => Err(chat_text::usage(lang, Usage::Plugin).to_owned()),
                };
                match result {
                    Ok(text) => writeln!(out, "{text}")?,
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err))?,
                }
            }
            Input::Tasks => {
                let result = if settings.interactive {
                    chat_tasks::menu(&settings.repo, lang)
                } else {
                    Ok((
                        chat_tasks::render(lang, &chat_tasks::scan(&settings.repo)),
                        None,
                    ))
                };
                match result {
                    Ok((text, change)) => {
                        writeln!(out, "{text}")?;
                        if let Some(change) = change {
                            // A task change is already on disk, so a hook can only warn here.
                            fire_and_print(
                                out,
                                &settings,
                                lang,
                                "task.changed",
                                &[
                                    ("id", change.id.as_str()),
                                    ("from", change.from.as_str()),
                                    ("to", change.to.as_str()),
                                ],
                            )?;
                        }
                    }
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err))?,
                }
            }
            Input::Goal(rest) => {
                if rest.is_empty() {
                    let text = match &goal {
                        Some(current) => chat_text::goal_set(lang, current),
                        None => chat_text::goal_none(lang).to_owned(),
                    };
                    writeln!(out, "{text}")?;
                } else if rest == "clear" {
                    goal = None;
                    writeln!(out, "{}", chat_text::goal_cleared(lang))?;
                } else {
                    writeln!(out, "{}", chat_text::goal_set(lang, &rest))?;
                    goal = Some(rest);
                }
            }
            Input::Compact => {
                if transcript.is_empty() {
                    writeln!(out, "{}", chat_text::nothing_to_compact(lang))?;
                    continue;
                }
                let request = Request {
                    prompt: String::new(),
                    max_output_tokens: settings.max_tokens,
                    timeout: settings.timeout,
                    dir: String::new(),
                };
                let mut messages = vec![Message::system(
                    "Ringkas percakapan berikut menjadi catatan singkat. Pertahankan fakta, \
                     keputusan, dan langkah berikutnya. Jawab dalam bahasa percakapan."
                        .to_owned(),
                )];
                messages.extend(transcript.history());
                messages.push(Message::user("Ringkas percakapan di atas.".to_owned()));
                match delegate::run_messages(provider.as_ref(), &messages, &request) {
                    Ok(response) => {
                        let turns = transcript.len();
                        transcript.clear();
                        transcript.push(Role::User, "Ringkasan percakapan sebelumnya:");
                        transcript.push(Role::Assistant, response.text.clone());
                        writeln!(out, "{}", chat_text::compacted(lang, turns))?;
                        writeln!(out, "{}", response.text)?;
                    }
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                }
            }
            Input::Providers => {
                if settings.providers.is_empty() {
                    writeln!(out, "{}", chat_text::no_providers(lang))?;
                }
                for entry in &settings.providers {
                    writeln!(
                        out,
                        "{}",
                        chat_text::provider_line(
                            lang,
                            &entry.name,
                            entry.enabled,
                            entry.billable,
                            entry.name == provider.name(),
                        )
                    )?;
                }
            }
            Input::Lsp(rest) => {
                let (action, target) = rest.split_once(' ').unwrap_or((rest.as_str(), ""));
                let result = chat_lsp::run(
                    &settings.lsp_servers,
                    &settings.approved,
                    &settings.repo,
                    action,
                    target.trim(),
                );
                match result {
                    Ok(text) => writeln!(out, "{text}")?,
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err))?,
                }
            }
            Input::Debug => {
                let repo = settings.repo.display().to_string();
                let timeout = format!("{:?}", settings.timeout);
                let facts = chat_text::DebugFacts {
                    version: env!("CARGO_PKG_VERSION"),
                    provider: provider.name(),
                    role: &active.name,
                    repo: &repo,
                    agent: &settings.agent_name,
                    turns: transcript.len(),
                    goal_set: goal.is_some(),
                    hooks: settings.hooks.len(),
                    mcp_servers: settings.mcp_servers.len(),
                    plugins: settings.plugins.len(),
                    roles: settings.roles.len(),
                    peers: settings.peers.len(),
                    advisor: settings.advisor.as_deref(),
                    council: settings.council.len(),
                    timeout: &timeout,
                    max_tokens: settings.max_tokens,
                };
                writeln!(out, "{}", chat_text::debug_report(lang, &facts))?;
            }
            Input::Advisor(question) => {
                if question.is_empty() {
                    writeln!(out, "{}", chat_text::advisor_usage(lang))?;
                    continue;
                }
                let Some(name) = settings.advisor.clone() else {
                    writeln!(out, "{}", chat_text::advisor_not_set(lang))?;
                    continue;
                };
                let advisor = match open(&name) {
                    Ok(advisor) => advisor,
                    Err(err) => {
                        writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?;
                        continue;
                    }
                };
                // The advisor sees the whole conversation so far, then the question.
                let mut messages = vec![Message::system(
                    "Anda adalah reviewer. Tinjau percakapan berikut dan jawab pertanyaan \
                     terakhir dengan jujur: sebutkan risiko, celah, dan langkah yang disarankan."
                        .to_owned(),
                )];
                messages.extend(transcript.history());
                messages.push(Message::user(question));
                let request = Request {
                    prompt: String::new(),
                    max_output_tokens: settings.max_tokens,
                    timeout: settings.timeout,
                    dir: String::new(),
                };
                match delegate::run_messages(advisor.as_ref(), &messages, &request) {
                    Ok(response) => {
                        writeln!(out, "[advisor {name}]")?;
                        writeln!(out, "{}", response.text)?;
                    }
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                }
            }
            Input::Council(question) => {
                if question.is_empty() {
                    writeln!(out, "{}", chat_text::council_usage(lang))?;
                    continue;
                }
                if settings.council.is_empty() {
                    writeln!(out, "{}", chat_text::council_empty(lang))?;
                    continue;
                }
                let request = Request {
                    prompt: String::new(),
                    max_output_tokens: settings.max_tokens,
                    timeout: settings.timeout,
                    dir: String::new(),
                };
                // Each member answers alone, without the conversation, so the answers stay independent.
                for name in settings.council.clone() {
                    let member = match open(&name) {
                        Ok(member) => member,
                        Err(err) => {
                            writeln!(out, "[{name}] {}", chat_text::error(lang, &err.to_string()))?;
                            continue;
                        }
                    };
                    let messages = vec![Message::user(question.clone())];
                    match delegate::run_messages(member.as_ref(), &messages, &request) {
                        Ok(response) => {
                            writeln!(out, "[{name}]")?;
                            writeln!(out, "{}", response.text)?;
                        }
                        Err(err) => {
                            writeln!(out, "[{name}] {}", chat_text::error(lang, &err.to_string()))?
                        }
                    }
                }
            }
            Input::Pending => match pending.menu() {
                Ok(text) => writeln!(out, "{text}")?,
                Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
            },
            Input::Pick(rest) => match chat_pending::parse_pick(&rest) {
                Ok((id, choice)) => match pending.answer(&id, choice) {
                    Ok(text) => writeln!(out, "{text}")?,
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                },
                Err(kind) => writeln!(out, "{}", chat_text::usage(lang, kind))?,
            },
            Input::Decisions(text) => {
                if text.is_empty() {
                    match pending.decisions_menu() {
                        Ok(list) => writeln!(out, "{list}")?,
                        Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                    }
                } else if let Some(rest) = text.strip_prefix("update ") {
                    match rest.trim().split_once(' ') {
                        Some((id, new_text)) if !new_text.trim().is_empty() => {
                            match pending.update(id, new_text.trim()) {
                                Ok(done) => writeln!(out, "{done}")?,
                                Err(err) => {
                                    writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?
                                }
                            }
                        }
                        _ => writeln!(out, "{}", chat_text::usage(lang, Usage::DecisionsUpdate))?,
                    }
                } else if text == "delete" {
                    writeln!(out, "{}", chat_text::usage(lang, Usage::DecisionsDelete))?;
                } else if let Some(id) = text.strip_prefix("delete ") {
                    match pending.archive(id.trim()) {
                        Ok(done) => writeln!(out, "{done}")?,
                        Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                    }
                } else {
                    let decision = Decision {
                        title: text.clone(),
                        chosen: text,
                        why: String::new(),
                    };
                    match record(&decision) {
                        Ok(saved) => {
                            writeln!(out, "{}", chat_text::decision_saved(lang, saved.trim()))?;
                            fire_and_print(
                                out,
                                &settings,
                                lang,
                                "decision.saved",
                                &[("saved", saved.trim())],
                            )?;
                        }
                        Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                    }
                }
            }
            Input::Decision(rest) => match parse_decision(&rest) {
                Ok(decision) => match record(&decision) {
                    Ok(saved) => {
                        writeln!(out, "{}", chat_text::decision_saved(lang, saved.trim()))?;
                        fire_and_print(
                            out,
                            &settings,
                            lang,
                            "decision.saved",
                            &[("saved", saved.trim())],
                        )?;
                    }
                    Err(err) => writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?,
                },
                Err(kind) => writeln!(out, "{}", chat_text::usage(lang, kind))?,
            },
            Input::Message(text) => {
                let request = Request {
                    prompt: String::new(),
                    max_output_tokens: settings.max_tokens,
                    timeout: settings.timeout,
                    dir: String::new(),
                };
                if let Some(reason) = fire_and_print(
                    out,
                    &settings,
                    lang,
                    "message.before_send",
                    &[("text", &text)],
                )? {
                    writeln!(out, "{}", chat_text::error(lang, &reason))?;
                    continue;
                }
                let (system, _) = chat_roles::system_text(&active, &settings.skills_dir);
                let mut messages: Vec<Message> = system.into_iter().map(Message::system).collect();
                if let Some(goal) = &goal {
                    messages.push(Message::system(format!("Goal sesi ini: {goal}")));
                }
                messages.extend(transcript.messages(&text));
                // Text goes out as the provider produces it, so a long reply shows while it is written.
                let mut printed = false;
                let result = delegate::run_messages_streaming(
                    provider.as_ref(),
                    &messages,
                    &request,
                    &mut |chunk| {
                        printed |= !chunk.is_empty();
                        let _ = out.write_all(chunk.as_bytes());
                        let _ = out.flush();
                    },
                );
                match result {
                    Ok(response) => {
                        writeln!(out)?;
                        fire_and_print(
                            out,
                            &settings,
                            lang,
                            "message.after_reply",
                            &[("text", &text), ("reply", &response.text)],
                        )?;
                        transcript.push(Role::User, text);
                        transcript.push(Role::Assistant, response.text);
                    }
                    Err(err) => {
                        if printed {
                            writeln!(out)?;
                        }
                        writeln!(out, "{}", chat_text::error(lang, &err.to_string()))?;
                    }
                }
            }
        }
        if settings.turn_marks {
            out.write_all(&[TURN_MARK])?;
        }
        out.flush()?;
    }
    fire_and_print(out, &settings, lang, "session.end", &[])?;
    Ok(())
}

pub fn run(args: &Args) -> CliResult {
    let settings_path = dirs::home_dir()
        .ok_or("home directory is not available")?
        .join(".plaesy")
        .join("settings.json");
    let lang = chat_text::resolve(args.lang.as_deref(), &settings_path)?;
    let mut hooks = chat_hooks::parse_hooks(
        &chat_hooks::read_settings(&args.path.join(".plaesy").join("settings.json")),
        chat_hooks::Scope::Repo,
    )?;
    hooks.extend(chat_hooks::parse_hooks(
        &chat_hooks::read_settings(&settings_path),
        chat_hooks::Scope::User,
    )?);
    let approved = chat_hooks::load_approved(&settings_path.with_file_name("approved-hooks.json"));
    let roles = chat_roles::merge(
        chat_roles::parse_roles(&chat_hooks::read_settings(&settings_path))?,
        chat_roles::parse_roles(&chat_hooks::read_settings(
            &args.path.join(".plaesy").join("settings.json"),
        ))?,
    );
    let role_name = args
        .role
        .clone()
        .unwrap_or_else(|| chat_roles::DEFAULT_ROLE.to_owned());
    let active_role =
        chat_roles::find(&roles, &role_name).ok_or_else(|| format!("unknown role {role_name}"))?;
    let home = dirs::home_dir().ok_or("home directory is not available")?;
    let skills_dir = super::skills::store_dir(&home);
    let peers: Vec<String> = chat_hooks::read_settings(&settings_path)
        .get("agent")
        .and_then(|agent| agent.get("peers"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let agent_name = args.agent.clone().unwrap_or_else(|| {
        chat_hooks::read_settings(&settings_path)
            .get("agent")
            .and_then(|agent| agent.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("chat")
            .to_owned()
    });
    let (mut plugins, mut problems) = chat_plugins::discover(
        &home.join(".plaesy").join("plugins"),
        chat_hooks::Scope::User,
    );
    let (repo_plugins, repo_problems) = chat_plugins::discover(
        &args.path.join(".plaesy").join("plugins"),
        chat_hooks::Scope::Repo,
    );
    plugins.extend(repo_plugins);
    problems.extend(repo_problems);
    for problem in &problems {
        eprintln!("plugin skipped: {problem}");
    }
    let mut mcp_servers = chat_mcp::parse_servers(
        &chat_hooks::read_settings(&args.path.join(".plaesy").join("settings.json")),
        chat_hooks::Scope::Repo,
    )?;
    mcp_servers.extend(chat_mcp::parse_servers(
        &chat_hooks::read_settings(&settings_path),
        chat_hooks::Scope::User,
    )?);
    let cfg = super::delegate::load_delegate_config(&args.path)?;
    let timeout = crate::reach::duration::parse(&args.timeout)
        .ok_or_else(|| format!("invalid --timeout {:?}", args.timeout))?;
    let getenv: Getenv = Arc::new(|name: &str| std::env::var(name).ok());
    let name = if !args.provider.is_empty() {
        args.provider.clone()
    } else if let Some(provider_name) = &active_role.provider {
        provider_name.clone()
    } else {
        default_provider(&cfg)?
    };
    let mut lsp_servers = chat_lsp::parse_servers(
        &chat_hooks::read_settings(&args.path.join(".plaesy").join("settings.json")),
        chat_hooks::Scope::Repo,
    )?;
    lsp_servers.extend(chat_lsp::parse_servers(
        &chat_hooks::read_settings(&settings_path),
        chat_hooks::Scope::User,
    )?);
    // `chat.advisor` names the reviewer for /advisor; the repository setting wins over the user's.
    let advisor = [
        args.path.join(".plaesy").join("settings.json"),
        settings_path.clone(),
    ]
    .iter()
    .find_map(|path| {
        chat_hooks::read_settings(path)
            .get("chat")
            .and_then(|chat| chat.get("advisor"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    });
    let provider = open_provider(&cfg, &name, args.confirm, &getenv)?;
    let open = |next: &str| open_provider(&cfg, next, args.confirm, &getenv);

    // Without --vault, the menus and saved decisions use the vault in settings (`watch_dir`).
    let vault = args.vault.clone().or_else(|| {
        chat_hooks::read_settings(&settings_path)
            .get("watch_dir")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
    });
    let record_vault = vault.clone();
    let record = move |decision: &Decision| record_decision(decision, record_vault.as_deref());

    // The menus need a terminal and the line prompt; the full-screen view draws its own screen.
    let interactive = std::io::IsTerminal::is_terminal(&io::stdin()) && !args.tui;
    let project = args.project.clone();
    let settings = ChatSettings {
        timeout,
        max_tokens: args.max_tokens,
        lang,
        repo: args.path.clone(),
        interactive: std::io::IsTerminal::is_terminal(&io::stdin()),
        hooks: hooks.clone(),
        approved: approved.clone(),
        mcp_servers: mcp_servers.clone(),
        roles: roles.clone(),
        role: active_role.name.clone(),
        skills_dir: skills_dir.clone(),
        plugins: plugins.clone(),
        agent_name: agent_name.clone(),
        peers: peers.clone(),
        advisor: advisor.clone(),
        council: cfg
            .providers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.name.clone())
            .collect(),
        lsp_servers: lsp_servers.clone(),
        providers: cfg
            .providers
            .iter()
            .map(|p| ProviderEntry {
                name: p.name.clone(),
                enabled: p.enabled,
                billable: p.billable,
            })
            .collect(),
        turn_marks: args.tui,
    };
    if args.tui {
        let confirm = args.confirm;
        let repo = args.path.display().to_string();
        chat_tui::run(lang, &repo, &name, |input, out| {
            let open = |next: &str| open_provider(&cfg, next, confirm, &getenv);
            let record_vault = vault.clone();
            let record =
                move |decision: &Decision| record_decision(decision, record_vault.as_deref());
            let pending = chat_pending::MemoryPending {
                vault: vault.clone(),
                project: project.clone(),
                interactive,
                lang,
            };
            let _ = converse(provider, &open, &record, &pending, settings, input, out);
        })?;
        return Ok(());
    }
    println!("{}", chat_text::banner(lang, &name));
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    converse(
        provider,
        &open,
        &record,
        &chat_pending::MemoryPending {
            vault,
            project,
            interactive,
            lang,
        },
        settings,
        io::BufReader::new(stdin),
        &mut stdout,
    )
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;
