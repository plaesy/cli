//! Agent-to-agent messages for `plaesy chat`, carried by the local relay (`plaesy relay`).
//!
//! A message body is a small JSON envelope: `{"depth": n, "text": "..."}`. Depth counts hops, so a
//! chain of replies cannot grow without limit: a reply is refused past [`MAX_DEPTH`]. Text is capped
//! at [`MAX_TEXT`] bytes. Sends use a deterministic id, so a retry is idempotent in the relay.
//!
//! Incoming messages are shown, never acted on. Acting on another agent's text is a decision for
//! the person, not for the chat.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::Watcher;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

pub const MAX_DEPTH: u32 = 3;
pub const MAX_TEXT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub depth: u32,
    pub text: String,
}

/// A body in the envelope form, or plain text (depth 0) when it is not JSON.
pub fn decode(body: &str) -> Envelope {
    #[derive(Deserialize)]
    struct Raw {
        depth: u32,
        text: String,
    }
    match serde_json::from_str::<Raw>(body) {
        Ok(raw) => Envelope {
            depth: raw.depth,
            text: raw.text,
        },
        Err(_) => Envelope {
            depth: 0,
            text: body.to_owned(),
        },
    }
}

pub fn encode(depth: u32, text: &str) -> String {
    json!({"depth": depth, "text": text}).to_string()
}

/// Refuses text over the size cap and hops past the depth cap, before anything is sent.
pub fn check_outgoing(text: &str, depth: u32) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("message text is empty".to_owned());
    }
    if text.len() > MAX_TEXT {
        return Err(format!("message is over {MAX_TEXT} bytes"));
    }
    if depth > MAX_DEPTH {
        return Err(format!("reply depth {depth} is over the limit {MAX_DEPTH}"));
    }
    Ok(())
}

/// A stable id for one message, so repeating a send with the same id is idempotent.
pub fn new_id(from: &str, to: &str, text: &str, nanos: u128) -> String {
    let mut hasher = Sha256::new();
    hasher.update(from.as_bytes());
    hasher.update([0]);
    hasher.update(to.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    hasher.update(nanos.to_le_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("a2a-{hex}")
}

pub fn nanos_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Incoming {
    pub id: String,
    pub sender: String,
    pub body: String,
}

pub fn parse_inbox(raw: &str) -> Result<Vec<Incoming>, String> {
    serde_json::from_str(raw).map_err(|err| format!("relay inbox: {err}"))
}

/// Runs `plaesy relay <args>` in the repository, so the relay database is the repository's.
pub fn run_relay(repo: &Path, args: &[String]) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let output = Command::new(exe)
        .arg("relay")
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

pub fn send_args(from: &str, to: &str, id: &str, body: &str) -> Vec<String> {
    vec![
        "send".to_owned(),
        "--id".to_owned(),
        id.to_owned(),
        "--from".to_owned(),
        from.to_owned(),
        "--to".to_owned(),
        to.to_owned(),
        body.to_owned(),
    ]
}

pub fn inbox_args(agent: &str) -> Vec<String> {
    vec!["inbox".to_owned(), "--agent".to_owned(), agent.to_owned()]
}

pub fn ack_args(id: &str, agent: &str) -> Vec<String> {
    vec![
        "ack".to_owned(),
        "--id".to_owned(),
        id.to_owned(),
        "--agent".to_owned(),
        agent.to_owned(),
    ]
}

/// Shown to the person: sender, depth, and text, with the id needed to reply or acknowledge.
pub fn render_incoming(items: &[Incoming]) -> String {
    if items.is_empty() {
        return "no messages".to_owned();
    }
    items
        .iter()
        .map(|item| {
            let envelope = decode(&item.body);
            format!(
                "{} from {} (depth {}): {}",
                item.id, item.sender, envelope.depth, envelope.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How often the listener checks the relay for new messages.
pub const LISTEN_INTERVAL: Duration = Duration::from_secs(5);

/// Starts a background listener for `agent`. It polls the relay and passes each new message to
/// `show`, once. Messages are shown, never acted on. The thread ends with the process; relay errors
/// are ignored, so a missing relay only stops the listener, not the chat.
pub fn spawn_listener<F>(repo: PathBuf, agent: String, interval: Duration, mut show: F)
where
    F: FnMut(&str) + Send + 'static,
{
    thread::spawn(move || {
        let (wake_tx, wake_rx) = mpsc::channel::<()>();
        // Wake on any write to the repository's `.plaesy` folder, where the relay database lives.
        // If the watcher cannot start (for example the folder does not exist yet), the loop falls
        // back to checking every `interval`.
        let watch_dir = repo.join(".plaesy");
        let _watcher = notify::recommended_watcher(move |_event: notify::Result<notify::Event>| {
            let _ = wake_tx.send(());
        })
        .and_then(|mut watcher| {
            watcher.watch(&watch_dir, notify::RecursiveMode::NonRecursive)?;
            Ok(watcher)
        })
        .ok();
        let mut seen: HashSet<String> = HashSet::new();
        show_new(&repo, &agent, &mut seen, &mut show);
        loop {
            match wake_rx.recv_timeout(interval) {
                Ok(()) => {
                    // A burst of writes means one check; drain the rest before reading the inbox.
                    while wake_rx.try_recv().is_ok() {}
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                // The watcher is gone; keep checking on the interval instead of spinning.
                Err(mpsc::RecvTimeoutError::Disconnected) => thread::sleep(interval),
            }
            show_new(&repo, &agent, &mut seen, &mut show);
        }
    });
}

/// Reads the inbox once and passes the messages not seen before to `show`.
fn show_new<F>(repo: &Path, agent: &str, seen: &mut HashSet<String>, show: &mut F)
where
    F: FnMut(&str),
{
    let fetched = run_relay(repo, &inbox_args(agent))
        .ok()
        .and_then(|raw| parse_inbox(&raw).ok());
    if let Some(items) = fetched {
        let fresh: Vec<Incoming> = items
            .into_iter()
            .filter(|item| seen.insert(item.id.clone()))
            .collect();
        if !fresh.is_empty() {
            show(&format!("[pesan baru]\n{}", render_incoming(&fresh)));
        }
    }
}

#[cfg(test)]
#[path = "chat_agents_test.rs"]
mod chat_agents_test;
