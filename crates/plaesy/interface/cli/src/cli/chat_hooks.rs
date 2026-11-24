//! Hooks for `plaesy chat`: a command that runs at a chat event. Event data reaches the command
//! as separate arguments; no shell is involved. Hooks from the repository run only after their
//! fingerprint is listed in `~/.plaesy/approved-hooks.json`, so a cloned repository cannot run
//! anything until its owner approves the exact hook. Hooks in the user's own settings are trusted.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Events a hook may name. `message.before_send` is the only one that may block.
pub const EVENTS: &[&str] = &[
    "session.start",
    "session.end",
    "message.before_send",
    "message.after_reply",
    "decision.saved",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Repo,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnFailure {
    Warn,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookSpec {
    pub event: String,
    pub command: String,
    pub args: Vec<String>,
    pub timeout: Duration,
    pub on_failure: OnFailure,
    pub scope: Scope,
}

#[derive(Debug, Deserialize)]
struct RawHook {
    event: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout: String,
    #[serde(default)]
    on_failure: Option<String>,
}

fn default_timeout() -> String {
    "10s".to_owned()
}

/// Reads a settings file. A missing or unreadable file is `null`, which has no hooks.
pub fn read_settings(path: &Path) -> serde_json::Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or(serde_json::Value::Null)
}

/// Hooks from a settings value (`"hooks": [...]`). Each one is checked: a known event, a valid
/// timeout, a known `on_failure`, and `block` only on `message.before_send`.
pub fn parse_hooks(value: &serde_json::Value, scope: Scope) -> Result<Vec<HookSpec>, String> {
    let Some(list) = value.get("hooks") else {
        return Ok(Vec::new());
    };
    let raws: Vec<RawHook> =
        serde_json::from_value(list.clone()).map_err(|err| format!("hooks: {err}"))?;
    raws.into_iter()
        .map(|raw| {
            if !EVENTS.contains(&raw.event.as_str()) {
                return Err(format!("unknown hook event {:?}", raw.event));
            }
            let timeout = crate::reach::duration::parse(&raw.timeout)
                .ok_or_else(|| format!("invalid hook timeout {:?}", raw.timeout))?;
            let on_failure = match raw.on_failure.as_deref() {
                None | Some("warn") => OnFailure::Warn,
                Some("block") => OnFailure::Block,
                Some(other) => return Err(format!("unknown on_failure {other:?}")),
            };
            if on_failure == OnFailure::Block && raw.event != "message.before_send" {
                return Err(format!(
                    "on_failure \"block\" is only allowed for message.before_send, not {}",
                    raw.event
                ));
            }
            Ok(HookSpec {
                event: raw.event,
                command: raw.command,
                args: raw.args,
                timeout,
                on_failure,
                scope,
            })
        })
        .collect()
}

/// A stable id for one hook: its event, command, arguments, timeout, and failure mode. Any change
/// to those changes the fingerprint, so an edited repository hook must be approved again.
pub fn fingerprint(spec: &HookSpec) -> String {
    let mut hasher = Sha256::new();
    hasher.update(spec.event.as_bytes());
    hasher.update([0]);
    hasher.update(spec.command.as_bytes());
    for arg in &spec.args {
        hasher.update([0]);
        hasher.update(arg.as_bytes());
    }
    hasher.update(spec.timeout.as_millis().to_le_bytes());
    hasher.update([match spec.on_failure {
        OnFailure::Warn => 1,
        OnFailure::Block => 2,
    }]);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Approved fingerprints, from `~/.plaesy/approved-hooks.json` (a JSON array of strings).
pub fn load_approved(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn is_trusted(spec: &HookSpec, approved: &[String]) -> bool {
    spec.scope == Scope::User || approved.iter().any(|fp| *fp == fingerprint(spec))
}

/// Replaces `{key}` with the event data value. One pass, so a value that itself contains
/// `{other}` is not expanded again. An unknown key stays as written.
pub fn expand_args(template: &[String], data: &BTreeMap<String, String>) -> Vec<String> {
    template
        .iter()
        .map(|arg| {
            let mut out = String::new();
            let mut rest = arg.as_str();
            while let Some(open) = rest.find('{') {
                out.push_str(&rest[..open]);
                let after = &rest[open + 1..];
                match after.find('}') {
                    Some(close) => match data.get(&after[..close]) {
                        Some(value) => {
                            out.push_str(value);
                            rest = &after[close + 1..];
                        }
                        None => {
                            out.push('{');
                            rest = after;
                        }
                    },
                    None => {
                        out.push_str(&rest[open..]);
                        rest = "";
                    }
                }
            }
            out.push_str(rest);
            out
        })
        .collect()
}

/// Runs one hook with no shell. The child is killed when it passes its timeout.
pub fn run(spec: &HookSpec, data: &BTreeMap<String, String>) -> Result<(), String> {
    let mut child = Command::new(&spec.command)
        .args(expand_args(&spec.args, data))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("hook {} ({}): {err}", spec.event, spec.command))?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|err| err.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => {
                return Err(format!("hook {} exited with {status}", spec.event));
            }
            None if started.elapsed() >= spec.timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "hook {} timed out after {:?}",
                    spec.event, spec.timeout
                ));
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// What the hooks of one event produced. `blocked` is set only by a failing `block` hook.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub warnings: Vec<String>,
    pub blocked: Option<String>,
}

/// Runs every hook of `event` that is trusted, in configuration order. An untrusted repository
/// hook is reported with its fingerprint so the owner can approve it.
pub fn fire(
    hooks: &[HookSpec],
    approved: &[String],
    event: &str,
    data: &BTreeMap<String, String>,
) -> Outcome {
    let mut outcome = Outcome::default();
    for spec in hooks.iter().filter(|spec| spec.event == event) {
        if !is_trusted(spec, approved) {
            outcome.warnings.push(format!(
                "hook {event} from the repository is not approved; add {} to approved-hooks.json to run it",
                fingerprint(spec)
            ));
            continue;
        }
        if let Err(err) = run(spec, data) {
            match spec.on_failure {
                OnFailure::Warn => outcome.warnings.push(err),
                OnFailure::Block => {
                    outcome.blocked = Some(err);
                    return outcome;
                }
            }
        }
    }
    outcome
}

#[cfg(test)]
#[path = "chat_hooks_test.rs"]
mod chat_hooks_test;
