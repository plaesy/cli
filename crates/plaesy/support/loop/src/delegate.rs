//! Sends a bounded prompt to another model through a provider chosen in
//! `.plaesy/config.json` (`delegate.providers`).
//!
//! Only enabled providers run. A billable provider also needs the caller to
//! confirm. A chain tries providers in order and falls through only on errors
//! that another provider may not repeat.
//! Port of `scripts/internal/delegate` (Go).

pub mod plan;
pub mod providers;
pub mod ratelimit;
pub mod retry;
pub mod usage;

use std::fmt;
use std::time::{Duration, Instant};

use serde::Serialize;
use thiserror::Error;

use crate::config::delegate::{DelegateConfig, DelegateProviderConfig};

/// Capability every request needs. A provider that lacks it is skipped.
pub const CAP_TEXT: &str = "text";

/// Longest `Retry-After` a chain will wait for before moving on.
pub const DEFAULT_MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// How a failure affects the rest of the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Timeout, network error, 5xx, or a CLI that did not run. The next provider may answer.
    Transient,
    /// HTTP 429. The chain waits for `Retry-After` when it is within the maximum, then falls through.
    RateLimited,
    /// Bad key, bad request, bad configuration, or a budget refusal. The chain stops.
    Terminal,
}

impl Class {
    /// Name used in output.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Transient => "transient",
            Self::RateLimited => "rate-limited",
            Self::Terminal => "terminal",
        }
    }
}

/// A provider failure with its class.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct DelegateError {
    pub class: Class,
    pub retry_after: Option<Duration>,
    pub message: String,
}

impl DelegateError {
    /// A transient failure.
    #[must_use]
    pub fn transient(message: impl Into<String>) -> Self {
        Self {
            class: Class::Transient,
            retry_after: None,
            message: message.into(),
        }
    }

    /// A terminal failure.
    #[must_use]
    pub fn terminal(message: impl Into<String>) -> Self {
        Self {
            class: Class::Terminal,
            retry_after: None,
            message: message.into(),
        }
    }
}

/// One prompt for a provider.
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub prompt: String,
    /// Zero means the provider default. CLI providers ignore it.
    pub max_output_tokens: u32,
    /// Zero means no limit beyond the process.
    pub timeout: Duration,
    /// Working directory for CLI providers. Empty means the process directory.
    pub dir: String,
}

/// What a provider returned for one call. Token counts are `None` when the provider does not report them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Response {
    pub provider: String,
    pub text: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub wall_ms: u64,
}

/// One way to reach a model.
pub trait Provider: Send + Sync {
    /// Configured provider name.
    fn name(&self) -> &str;
    /// What the provider can take. Text only unless overridden.
    fn capabilities(&self) -> &[&'static str] {
        &[CAP_TEXT]
    }
    /// Makes one call.
    ///
    /// # Errors
    ///
    /// A [`DelegateError`] classified for the chain.
    fn complete(&self, req: &Request) -> Result<ProviderReply, DelegateError>;
    /// Makes one call with a conversation, oldest message first. Providers that accept a message
    /// list override this. The default flattens the messages into one prompt, so single-prompt
    /// providers (CLI) keep working unchanged.
    ///
    /// # Errors
    ///
    /// A [`DelegateError`] classified for the chain.
    fn complete_messages(
        &self,
        messages: &[Message],
        req: &Request,
    ) -> Result<ProviderReply, DelegateError> {
        let flat = Request {
            prompt: flatten(messages),
            ..req.clone()
        };
        self.complete(&flat)
    }
    /// Like [`Provider::complete_messages`], but passes each piece of text to `on_text` as it
    /// arrives. The default makes one call and passes the whole reply once, so a provider that
    /// cannot stream still works.
    ///
    /// # Errors
    ///
    /// A [`DelegateError`] classified for the chain.
    fn complete_messages_streaming(
        &self,
        messages: &[Message],
        req: &Request,
        on_text: &mut dyn FnMut(&str),
    ) -> Result<ProviderReply, DelegateError> {
        let reply = self.complete_messages(messages, req)?;
        on_text(&reply.text);
        Ok(reply)
    }
}

/// One turn of a conversation. `role` is `user` or `assistant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

impl Message {
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user",
            content: content.into(),
        }
    }

    #[must_use]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant",
            content: content.into(),
        }
    }

    /// Instructions for the model, such as a role's system prompt. Sent first, never as a turn.
    #[must_use]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system",
            content: content.into(),
        }
    }
}

/// Renders messages as one completion-style prompt: `User: ...`, `Assistant: ...` blocks, then an
/// open `Assistant:` label when the last message is from the user.
#[must_use]
pub fn flatten(messages: &[Message]) -> String {
    let mut out = String::new();
    for message in messages {
        let label = match message.role {
            "assistant" => "Assistant",
            "system" => "System",
            _ => "User",
        };
        out.push_str(label);
        out.push_str(": ");
        out.push_str(&message.content);
        out.push_str("\n\n");
    }
    if messages.last().is_none_or(|m| m.role != "assistant") {
        out.push_str("Assistant:");
    }
    out
}

/// What a provider returns before the name and timing are added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReply {
    pub text: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// What a chain did with one provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attempt {
    pub provider: String,
    /// `answered`, `skipped` (no call made), or `failed` (a call was made and failed).
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<&'static str>,
    #[serde(skip_serializing_if = "is_zero")]
    pub waited_ms: u64,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// Returned when a chain produces no answer. It lists every attempt.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub struct ChainError {
    pub chain: String,
    /// True when a terminal error or an interruption ended the chain early.
    pub stopped: bool,
    pub attempts: Vec<Attempt>,
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.stopped {
            write!(
                f,
                "delegate chain {:?} stopped on a terminal error; no provider answered",
                self.chain
            )?;
        } else {
            write!(f, "delegate chain {:?}: no provider answered", self.chain)?;
        }
        for a in &self.attempts {
            write!(f, "\n  - {}: {}", a.provider, a.outcome)?;
            if let Some(class) = a.class {
                write!(f, " ({class})")?;
            }
            if !a.reason.is_empty() {
                write!(f, ": {}", a.reason)?;
            }
        }
        Ok(())
    }
}

/// Reports whether an entry may run now.
///
/// # Errors
///
/// Returns an error string when the entry is missing, disabled, or billable without confirmation.
pub fn select(
    cfg: &DelegateConfig,
    name: &str,
    confirm: bool,
) -> Result<DelegateProviderConfig, String> {
    let entry = cfg
        .providers
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            format!(
                "unknown provider {name:?} (configured: {})",
                names(cfg).join(", ")
            )
        })?;
    if !entry.enabled {
        return Err(format!(
            "provider {name:?} is disabled; set delegate.providers[].enabled to true in .plaesy/config.json"
        ));
    }
    if entry.billable && !confirm {
        return Err(format!(
            "provider {name:?} is billable (it charges per call); pass --confirm to run it"
        ));
    }
    Ok(entry.clone())
}

/// Configured provider names, in config order.
#[must_use]
pub fn names(cfg: &DelegateConfig) -> Vec<String> {
    cfg.providers.iter().map(|p| p.name.clone()).collect()
}

/// Builds a provider from its config entry. `getenv` reads API keys, which keeps
/// tests off the real environment.
///
/// # Errors
///
/// A terminal [`DelegateError`] for a missing field or an unknown type.
pub fn build(
    entry: &DelegateProviderConfig,
    getenv: providers::Getenv,
) -> Result<Box<dyn Provider>, DelegateError> {
    providers::build(entry, getenv)
}

/// Options for one chain run.
/// Turns a config entry into a provider.
pub type BuildFn<'a> =
    &'a dyn Fn(&DelegateProviderConfig) -> Result<Box<dyn Provider>, DelegateError>;
/// Runs before a call and may refuse it.
pub type CheckFn<'a> = &'a dyn Fn(&DelegateProviderConfig, &Request) -> Result<(), DelegateError>;

pub struct ChainOptions<'a> {
    /// Name shown in errors. Empty for a single provider.
    pub chain: String,
    /// Allows billable providers to run.
    pub confirm: bool,
    pub max_retry_after: Duration,
    /// Turns an entry into a provider.
    pub build: BuildFn<'a>,
    /// Runs before each call. An error stops the chain and sends nothing.
    pub check: Option<CheckFn<'a>>,
    /// Waits for `Retry-After`. Returns an error if interrupted.
    pub sleep: &'a dyn Fn(Duration) -> Result<(), String>,
}

/// Tries `names` in order and returns the first answer with the attempts made.
///
/// Transient errors fall through. A rate limit waits for `Retry-After` when it
/// fits within the maximum, then falls through. A terminal error stops the
/// chain. Disabled, unconfirmed billable, and text-incapable providers are skipped
/// without a call.
///
/// # Errors
///
/// [`ChainError`] when no provider answers.
pub fn run_chain(
    cfg: &DelegateConfig,
    names: &[String],
    req: &Request,
    opts: &ChainOptions<'_>,
) -> Result<(Response, Vec<Attempt>), ChainError> {
    let mut attempts: Vec<Attempt> = Vec::new();
    let fail = |attempts: Vec<Attempt>, stopped: bool| ChainError {
        chain: opts.chain.clone(),
        stopped,
        attempts,
    };

    for name in names {
        let entry = match select(cfg, name, opts.confirm) {
            Ok(entry) => entry,
            Err(reason) => {
                attempts.push(skipped(name, reason));
                continue;
            }
        };
        let provider = match (opts.build)(&entry) {
            Ok(provider) => provider,
            Err(e) => {
                attempts.push(failed(name, &e, Some(e.class)));
                return Err(fail(attempts, true));
            }
        };
        if !provider.capabilities().contains(&CAP_TEXT) {
            attempts.push(skipped(name, "provider lacks the text capability".into()));
            continue;
        }
        if let Some(check) = opts.check {
            if let Err(e) = check(&entry, req) {
                attempts.push(failed(name, &e, Some(e.class)));
                return Err(fail(attempts, true));
            }
        }

        let started = Instant::now();
        match provider.complete(req) {
            Ok(reply) => {
                attempts.push(Attempt {
                    provider: name.clone(),
                    outcome: "answered",
                    reason: String::new(),
                    class: None,
                    waited_ms: 0,
                });
                let response = Response {
                    provider: name.clone(),
                    text: reply.text,
                    input_tokens: reply.input_tokens,
                    output_tokens: reply.output_tokens,
                    wall_ms: started.elapsed().as_millis() as u64,
                };
                return Ok((response, attempts));
            }
            Err(e) => {
                let mut attempt = failed(name, &e, Some(e.class));
                if e.class == Class::Terminal {
                    attempts.push(attempt);
                    return Err(fail(attempts, true));
                }
                if e.class == Class::RateLimited {
                    if let Some(wait) = e.retry_after.filter(|w| !w.is_zero()) {
                        if wait > opts.max_retry_after {
                            attempt.reason.push_str(&format!(
                                "; retry-after {wait:?} exceeds the {:?} maximum, moving on",
                                opts.max_retry_after
                            ));
                        } else {
                            let slept_at = Instant::now();
                            if let Err(interrupt) = (opts.sleep)(wait) {
                                attempt
                                    .reason
                                    .push_str(&format!("; interrupted while waiting: {interrupt}"));
                                attempts.push(attempt);
                                return Err(fail(attempts, true));
                            }
                            attempt.waited_ms = slept_at.elapsed().as_millis() as u64;
                        }
                    }
                }
                attempts.push(attempt);
            }
        }
    }
    Err(fail(attempts, false))
}

fn skipped(name: &str, reason: String) -> Attempt {
    Attempt {
        provider: name.to_owned(),
        outcome: "skipped",
        reason,
        class: None,
        waited_ms: 0,
    }
}

fn failed(name: &str, e: &DelegateError, class: Option<Class>) -> Attempt {
    Attempt {
        provider: name.to_owned(),
        outcome: "failed",
        reason: e.message.clone(),
        class: class.map(Class::label),
        waited_ms: 0,
    }
}

/// Sleeps for `d`, used when no other waiter is injected.
///
/// # Errors
///
/// Never in practice. The signature matches [`ChainOptions::sleep`].
pub fn sleep_for(d: Duration) -> Result<(), String> {
    std::thread::sleep(d);
    Ok(())
}

/// Applies the request timeout around a provider call and stamps the wall time.
///
/// # Errors
///
/// The provider's error, unchanged.
pub fn run(provider: &dyn Provider, req: &Request) -> Result<Response, DelegateError> {
    let started = Instant::now();
    let reply = provider.complete(req)?;
    Ok(response_from(provider, reply, started))
}

/// Like [`run`], for a conversation. `messages` is the full history with the new user turn last.
///
/// # Errors
///
/// The provider's error, unchanged.
pub fn run_messages(
    provider: &dyn Provider,
    messages: &[Message],
    req: &Request,
) -> Result<Response, DelegateError> {
    let started = Instant::now();
    let reply = provider.complete_messages(messages, req)?;
    Ok(response_from(provider, reply, started))
}

/// Like [`run_messages`], but `on_text` receives the reply as it is generated.
///
/// # Errors
///
/// The provider's [`DelegateError`].
pub fn run_messages_streaming(
    provider: &dyn Provider,
    messages: &[Message],
    req: &Request,
    on_text: &mut dyn FnMut(&str),
) -> Result<Response, DelegateError> {
    let started = Instant::now();
    let reply = provider.complete_messages_streaming(messages, req, on_text)?;
    Ok(response_from(provider, reply, started))
}

fn response_from(provider: &dyn Provider, reply: ProviderReply, started: Instant) -> Response {
    Response {
        provider: provider.name().to_owned(),
        text: reply.text,
        input_tokens: reply.input_tokens,
        output_tokens: reply.output_tokens,
        wall_ms: started.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
#[path = "delegate_test.rs"]
mod delegate_test;
