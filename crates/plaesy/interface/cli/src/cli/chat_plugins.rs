//! Plugins for `plaesy chat`. A plugin is a folder with a `plugin.json` manifest. Its commands run
//! as separate processes, with arguments passed as words (no shell), a timeout, and a path that must
//! stay inside the plugin folder. Plugins live in `~/.plaesy/plugins` (trusted, the user's own) and
//! `.plaesy/plugins` in the repository (trusted only after approval). The manifest's permissions are
//! shown and covered by the approval, but they are not enforced by a sandbox: a plugin runs with the
//! user's rights. That limit is stated, not hidden.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::chat_hooks::Scope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCommand {
    pub name: String,
    /// Path relative to the plugin folder; never leaves it.
    pub command: String,
    pub help: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    pub name: String,
    pub version: String,
    pub dir: PathBuf,
    pub commands: Vec<PluginCommand>,
    pub permissions: Vec<String>,
    pub scope: Scope,
    /// The manifest text, which the fingerprint covers.
    pub manifest: String,
}

#[derive(Debug, Deserialize)]
struct RawCommand {
    name: String,
    command: String,
    #[serde(default)]
    help: String,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    commands: Vec<RawCommand>,
    #[serde(default)]
    permissions: Vec<String>,
}

/// A name is one plain folder-safe word: letters, digits, `-` and `_`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A command path must be relative and have no `..`, root, or drive component.
pub fn valid_relative(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        && !p.is_absolute()
}

/// Parses one manifest. The folder name must match the manifest name, so a folder cannot claim
/// another plugin's name.
pub fn parse_manifest(text: &str, dir: &Path, scope: Scope) -> Result<Plugin, String> {
    let raw: RawManifest =
        serde_json::from_str(text).map_err(|err| format!("{}: {err}", dir.display()))?;
    if !valid_name(&raw.name) {
        return Err(format!(
            "plugin name {:?} must be letters, digits, - or _",
            raw.name
        ));
    }
    let folder = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if folder != raw.name {
        return Err(format!(
            "folder {folder} does not match plugin name {}",
            raw.name
        ));
    }
    let mut commands = Vec::new();
    for command in raw.commands {
        if !valid_name(&command.name) {
            return Err(format!(
                "command name {:?} must be letters, digits, - or _",
                command.name
            ));
        }
        if !valid_relative(&command.command) {
            return Err(format!(
                "command path {:?} must be relative and stay in the plugin folder",
                command.command
            ));
        }
        commands.push(PluginCommand {
            name: command.name,
            command: command.command,
            help: command.help,
        });
    }
    Ok(Plugin {
        name: raw.name,
        version: raw.version,
        dir: dir.to_path_buf(),
        commands,
        permissions: raw.permissions,
        scope,
        manifest: text.to_owned(),
    })
}

/// Every valid plugin in `root/<name>/plugin.json`. A broken plugin is reported and skipped, so one
/// bad folder does not hide the others.
pub fn discover(root: &Path, scope: Scope) -> (Vec<Plugin>, Vec<String>) {
    let mut plugins = Vec::new();
    let mut problems = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return (plugins, problems);
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let Ok(text) = fs::read_to_string(dir.join("plugin.json")) else {
            continue;
        };
        match parse_manifest(&text, &dir, scope) {
            Ok(plugin) => plugins.push(plugin),
            Err(err) => problems.push(err),
        }
    }
    (plugins, problems)
}

/// A stable id for a plugin: its manifest text. Editing the manifest, including its permissions,
/// needs a new approval.
pub fn fingerprint(plugin: &Plugin) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"plugin\0");
    hasher.update(plugin.dir.to_string_lossy().as_bytes());
    hasher.update([0]);
    hasher.update(plugin.manifest.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn is_trusted(plugin: &Plugin, approved: &[String]) -> bool {
    plugin.scope == Scope::User || approved.iter().any(|fp| *fp == fingerprint(plugin))
}

/// The executable for a command, checked to stay inside the plugin folder.
pub fn command_path(plugin: &Plugin, command: &PluginCommand) -> Result<PathBuf, String> {
    if !valid_relative(&command.command) {
        return Err(format!("command path {:?} is not allowed", command.command));
    }
    Ok(plugin.dir.join(&command.command))
}

/// Runs an executable with arguments as separate words, captures stdout, and kills the process when
/// it passes the timeout. Returns stdout on success.
pub fn run_capture(
    program: &Path,
    args: &[String],
    dir: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("plugin {}: {err}", program.display()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "plugin: no stdout".to_owned())?;
    let reader = thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|err| err.to_string())? {
            Some(status) => {
                let text = reader.join().unwrap_or_default();
                if status.success() {
                    return Ok(text.trim().to_owned());
                }
                return Err(format!("plugin exited with {status}"));
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(format!("plugin timed out after {timeout:?}"));
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// `/plugins`: each plugin with its commands, or why it is not usable.
pub fn list(plugins: &[Plugin], approved: &[String]) -> String {
    if plugins.is_empty() {
        return "no plugins".to_owned();
    }
    let mut lines = Vec::new();
    for plugin in plugins {
        if !is_trusted(plugin, approved) {
            lines.push(format!(
                "{} {}: not approved; add {} to approved-hooks.json to run it",
                plugin.name,
                plugin.version,
                fingerprint(plugin)
            ));
            continue;
        }
        let commands: Vec<String> = plugin
            .commands
            .iter()
            .map(|c| {
                if c.help.is_empty() {
                    c.name.clone()
                } else {
                    format!("{} ({})", c.name, c.help)
                }
            })
            .collect();
        lines.push(format!(
            "{} {}: {}; permissions: {}",
            plugin.name,
            plugin.version,
            commands.join(", "),
            if plugin.permissions.is_empty() {
                "none".to_owned()
            } else {
                plugin.permissions.join(", ")
            }
        ));
    }
    lines.join("\n")
}

/// `/plugin run <plugin> <command> <args...>`. Refused unless the plugin is trusted and the command
/// exists in its manifest.
pub fn run(
    plugins: &[Plugin],
    approved: &[String],
    plugin_name: &str,
    command_name: &str,
    args: &[String],
    timeout: Duration,
) -> Result<String, String> {
    let plugin = plugins
        .iter()
        .find(|p| p.name == plugin_name)
        .ok_or_else(|| format!("no plugin {plugin_name}"))?;
    if !is_trusted(plugin, approved) {
        return Err(format!("plugin {plugin_name} is not approved"));
    }
    let command = plugin
        .commands
        .iter()
        .find(|c| c.name == command_name)
        .ok_or_else(|| format!("plugin {plugin_name} has no command {command_name}"))?;
    let program = command_path(plugin, command)?;
    run_capture(&program, args, &plugin.dir, timeout)
}

#[cfg(test)]
#[path = "chat_plugins_test.rs"]
mod chat_plugins_test;
