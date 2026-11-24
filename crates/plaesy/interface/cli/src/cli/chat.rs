//! `plaesy chat`: an interactive, multi-turn session with one configured provider.
//!
//! Provider calls are single-shot, so the transcript is rendered into each prompt. Switching
//! provider with `/provider <name>` keeps the transcript. Billable providers need `--confirm`.

use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::CliResult;
use crate::config::delegate::DelegateConfig;
use crate::delegate::providers::Getenv;
use crate::delegate::{self, flatten, Message, Provider, Request};

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
}

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
        _ => {}
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
    timeout: Duration,
    max_tokens: u32,
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
            Input::Message(text) => {
                let request = Request {
                    prompt: String::new(),
                    max_output_tokens: max_tokens,
                    timeout,
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

    println!("chat with {name}. /provider <name> switches, /clear resets, /exit quits.");
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    converse(
        provider,
        &open,
        timeout,
        args.max_tokens,
        stdin.lock(),
        &mut stdout,
    )
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;
