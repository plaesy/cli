//! Tools for `plaesy chat --tools`.
//!
//! The model asks for a tool by writing one fenced block whose info string is `tool`, holding a
//! JSON object with a `tool` name and its arguments. The host runs it, sends the result back as
//! the next message, and repeats until the model answers without a block. The text protocol works
//! with every provider, including the CLI ones that take one prompt.
//!
//! `read_file` and `list_dir` are always available. `write_file` and `run_command` run only with
//! `--allow-writes`; without it the model is told why the call was refused. Every path is confined
//! to the repository root: an absolute path, a `..` component, or a symlink that leaves the root is
//! refused. Output is cut to [`MAX_OUTPUT_BYTES`], a turn stops after [`MAX_ROUNDS`] calls, and a
//! command runs with no shell and is killed after [`COMMAND_TIMEOUT`].

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::delegate::{self, DelegateError, Message, Provider, Request, Response};

/// Tool calls one turn may make before the chat stops and reports the limit.
pub const MAX_ROUNDS: usize = 6;

/// Bytes of tool output sent back to the model. Longer output is cut at a character boundary.
pub const MAX_OUTPUT_BYTES: usize = 8_000;

/// Longest a `run_command` call may run before it is killed.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

const OPEN: &str = "```tool";
const CLOSE: &str = "```";

/// Where tools may act, and whether they may change anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    /// Repository root every path is confined to.
    pub root: PathBuf,
    /// True when `write_file` and `run_command` are allowed (`--allow-writes`).
    pub allow_writes: bool,
    /// True when `web_fetch` is allowed (`--web`). Off by default: it is the only tool that uses the network.
    pub web: bool,
}

/// Tells the model which tools exist and how to call them. Sent as a system message when tools are on.
#[must_use]
pub fn protocol(tools: &Tools) -> String {
    let allow_writes = tools.allow_writes;
    let mut text = String::from(
        "Kamu boleh memakai alat di repositori ini. Untuk memanggil alat, tulis satu blok \
         berikut dan berhenti:\n```tool\n{\"tool\":\"read_file\",\"path\":\"relatif/ke/root\"}\n```\n\
         atau {\"tool\":\"list_dir\",\"path\":\".\"}, atau {\"tool\":\"list_tasks\"} untuk daftar task. Path harus relatif terhadap root repositori, \
         tanpa `..`.",
    );
    text.push_str(" Juga bisa: {\"tool\":\"list_tasks\"}; dengan --allow-writes juga {\"tool\":\"create_task\",\"priority\":\"medium\",\"category\":\"chore\",\"title\":\"...\"} dan {\"tool\":\"schedule_once\"}.");
    if allow_writes {
        text.push_str(
            " Juga tersedia: {\"tool\":\"write_file\",\"path\":\"...\",\"content\":\"...\"} untuk \
             menulis berkas, dan {\"tool\":\"run_command\",\"command\":\"...\",\"args\":[...]} untuk \
             menjalankan satu program tanpa shell.",
        );
    } else {
        text.push_str(" Menulis berkas dan menjalankan perintah tidak diizinkan dalam sesi ini.");
    }
    if tools.web {
        text.push_str(" Juga bisa: {\"tool\":\"web_fetch\",\"url\":\"https://...\"} untuk membaca sebuah URL.");
    }
    text.push_str(
        " Hasil alat dikirim sebagai pesan berikutnya. Setelah hasilnya cukup, jawab biasa tanpa \
         blok alat.",
    );
    text
}

/// Finds the last `tool` block in `text` and parses its JSON. `None` when there is no block or
/// its body is not a JSON object.
#[must_use]
pub fn find_call(text: &str) -> Option<Value> {
    let start = text.rfind(OPEN)? + OPEN.len();
    let body = &text[start..];
    let end = body.find(CLOSE)?;
    let value: Value = serde_json::from_str(body[..end].trim()).ok()?;
    value.is_object().then_some(value)
}

/// One file state saved before a write: the bytes it held, or None when the file did not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub path: PathBuf,
    pub previous: Option<Vec<u8>>,
}

/// Undoes the last write: restores the saved bytes, or removes a file the write created.
/// Returns a line for the chat.
pub fn undo_last(undo: &mut Vec<Checkpoint>) -> Result<String, String> {
    let Some(point) = undo.pop() else {
        return Err("tidak ada tulisan yang bisa dibatalkan".to_owned());
    };
    let rel = point.path.display().to_string();
    match point.previous {
        Some(bytes) => fs::write(&point.path, bytes).map_err(|e| format!("{rel}: {e}"))?,
        None => fs::remove_file(&point.path).map_err(|e| format!("{rel}: {e}"))?,
    }
    Ok(format!("dibatalkan: {rel}"))
}

#[cfg(test)]
#[must_use]
pub fn execute(tools: &Tools, call: &Value) -> String {
    execute_with(tools, call, &mut Vec::new())
}

/// Runs one call and returns its result as text. Errors are returned as text too, so the model
/// can read them and carry on.
#[must_use]
pub fn execute_with(tools: &Tools, call: &Value, undo: &mut Vec<Checkpoint>) -> String {
    let name = call.get("tool").and_then(Value::as_str).unwrap_or("");
    let text = |key: &str| {
        call.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let result = match name {
        "read_file" => read_file(&tools.root, &text("path")),
        "create_task" if !tools.allow_writes => Err(
            "tidak diizinkan dalam sesi ini; sesi perlu dijalankan dengan --allow-writes"
                .to_owned(),
        ),
        "schedule_once" if !tools.allow_writes => Err(
            "tidak diizinkan dalam sesi ini; sesi perlu dijalankan dengan --allow-writes"
                .to_owned(),
        ),
        "web_fetch" if !tools.web => {
            Err("web tidak diaktifkan; sesi perlu dijalankan dengan --web".to_owned())
        }
        "create_task" => run_plaesy(
            &tools.root,
            &[
                "tasks",
                "create",
                &text("priority"),
                &text("category"),
                &text("title"),
            ],
        ),
        "schedule_once" => run_plaesy(&tools.root, &["memory", "schedule", "once"]),
        "web_fetch" => web_fetch(&text("url")),
        "list_tasks" => Ok(super::chat_tasks::render(
            super::chat_text::Lang::En,
            &super::chat_tasks::scan(&tools.root),
        )),
        "list_dir" => list_dir(&tools.root, &text("path")),
        "write_file" | "run_command" if !tools.allow_writes => Err(
            "tidak diizinkan dalam sesi ini; sesi perlu dijalankan dengan --allow-writes"
                .to_owned(),
        ),
        "write_file" => write_file(&tools.root, &text("path"), &text("content"), undo),
        "run_command" => {
            let args: Vec<String> = call
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            run_command(&tools.root, &text("command"), &args)
        }
        other => Err(format!("alat tidak dikenal: {other}")),
    };
    match result {
        Ok(text) => truncate(&text),
        Err(err) => format!("error: {err}"),
    }
}

/// One-line note shown in the chat when a call runs.
#[must_use]
pub fn describe(call: &Value) -> String {
    let name = call.get("tool").and_then(Value::as_str).unwrap_or("?");
    let target = call
        .get("path")
        .or_else(|| call.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("[alat] {name} {target}")
}

/// Resolves `rel` under `root`. Refuses absolute paths, `..`, and anything whose real location
/// lies outside the canonical root.
///
/// # Errors
///
/// A message naming the refused path, or the I/O error when the path does not exist.
pub fn confine(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim();
    let candidate = if rel.is_empty() { "." } else { rel };
    check_relative(candidate)?;
    let root = root
        .canonicalize()
        .map_err(|e| format!("root tidak bisa dibaca: {e}"))?;
    let full = root
        .join(candidate)
        .canonicalize()
        .map_err(|e| format!("{candidate}: {e}"))?;
    if !full.starts_with(&root) {
        return Err(format!("path di luar root: {candidate}"));
    }
    Ok(full)
}

/// Like [`confine`], for a file that may not exist yet: its parent folder must exist under the
/// root, and an existing target must also pass [`confine`].
fn confine_new(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim();
    check_relative(rel)?;
    let path = Path::new(rel);
    let name = path
        .file_name()
        .ok_or_else(|| format!("path tanpa nama berkas: {rel}"))?;
    let parent = path
        .parent()
        .map_or_else(String::new, |p| p.display().to_string());
    let folder = confine(root, &parent)?;
    let target = folder.join(name);
    if target.symlink_metadata().is_ok() {
        // Existing target: it must resolve inside the root too, so a link cannot redirect the write.
        return confine(root, rel);
    }
    Ok(target)
}

fn check_relative(candidate: &str) -> Result<(), String> {
    let path = Path::new(candidate);
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(format!("path harus relatif tanpa `..`: {candidate}"));
    }
    Ok(())
}

fn read_file(root: &Path, rel: &str) -> Result<String, String> {
    let full = confine(root, rel)?;
    if full.is_dir() {
        return Err(format!("{rel} adalah folder; pakai list_dir"));
    }
    fs::read_to_string(&full).map_err(|e| format!("{rel}: {e}"))
}

fn list_dir(root: &Path, rel: &str) -> Result<String, String> {
    let full = confine(root, rel)?;
    if !full.is_dir() {
        return Err(format!("{rel} bukan folder"));
    }
    let mut names: Vec<String> = fs::read_dir(&full)
        .map_err(|e| format!("{rel}: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    names.sort();
    Ok(names.join("\n"))
}

fn write_file(
    root: &Path,
    rel: &str,
    content: &str,
    undo: &mut Vec<Checkpoint>,
) -> Result<String, String> {
    let target = confine_new(root, rel)?;
    if target.is_dir() {
        return Err(format!("{rel} adalah folder"));
    }
    undo.push(Checkpoint {
        path: target.clone(),
        previous: fs::read(&target).ok(),
    });
    fs::write(&target, content).map_err(|e| format!("{rel}: {e}"))?;
    Ok(format!("ditulis {} byte ke {rel}", content.len()))
}

/// Runs a program directly, with no shell, in the root. Output is the exit code and both streams.
fn run_command(root: &Path, command: &str, args: &[String]) -> Result<String, String> {
    if command.trim().is_empty() || command.contains(['/', '\\']) {
        return Err("command harus nama program tanpa path".to_owned());
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("root tidak bisa dibaca: {e}"))?;
    let mut child = Command::new(command)
        .args(args)
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{command}: {e}"))?;
    // Each pipe is drained on its own thread, so a chatty program cannot block on a full pipe.
    let out_reader = drain(child.stdout.take());
    let err_reader = drain(child.stderr.take());
    let started = std::time::Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if started.elapsed() >= COMMAND_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{command} dihentikan setelah {} detik",
                    COMMAND_TIMEOUT.as_secs()
                ));
            }
            None => thread::sleep(Duration::from_millis(50)),
        }
    };
    let out = out_reader.recv().unwrap_or_default();
    let err = err_reader.recv().unwrap_or_default();
    Ok(format!("exit={:?}\n{out}{err}", status.code()))
}

/// Reads a pipe to its end on a thread; the receiver gets the text once the pipe closes.
fn drain(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut buf);
        }
        let _ = sender.send(buf);
    });
    receiver
}

fn truncate(text: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text.to_owned();
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[dipotong]", &text[..end])
}

/// Runs the model with tool rounds. `on_note` receives one line per call before its result runs.
/// The returned response is the reply that carries no tool block.
///
/// # Errors
///
/// The provider's error, or a terminal error when the model still asks for tools after
/// [`MAX_ROUNDS`] calls.
pub fn run_turn(
    provider: &dyn Provider,
    messages: &[Message],
    req: &Request,
    tools: &Tools,
    undo: &mut Vec<Checkpoint>,
    on_note: &mut dyn FnMut(&str),
) -> Result<Response, DelegateError> {
    let mut conversation = messages.to_vec();
    for _ in 0..MAX_ROUNDS {
        let response = delegate::run_messages(provider, &conversation, req)?;
        let Some(call) = find_call(&response.text) else {
            return Ok(response);
        };
        on_note(&describe(&call));
        let result = execute_with(tools, &call, undo);
        conversation.push(Message::assistant(response.text.clone()));
        conversation.push(Message::user(format!("Hasil alat:\n{result}")));
    }
    Err(DelegateError::terminal(format!(
        "batas {MAX_ROUNDS} panggilan alat tercapai dalam satu giliran"
    )))
}

#[cfg(test)]
#[path = "chat_tools_test.rs"]
mod chat_tools_test;

/// Runs this same binary with `args` in `root`, the way `/tasks` runs its commands. Output or the
/// error text is returned as a tool result.
fn run_plaesy(root: &Path, args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    super::chat_tasks::run_command(root, &args)
}

/// Fetches a URL for `web_fetch`. `https` for any host, `http` only for loopback, the same rule as
/// webhook URLs. The body is read as text and cut like other tool output.
fn web_fetch(url: &str) -> Result<String, String> {
    plaesy_common::chat_webhook::validate_url(url).map_err(|e| e.to_string())?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(COMMAND_TIMEOUT))
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|e| format!("{url}: {e}"))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("membaca balasan: {e}"))?;
    Ok(format!("status={status}\n{body}"))
}
