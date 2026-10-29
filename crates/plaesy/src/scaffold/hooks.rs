//! Installs the advisory design-lint hook for each AI platform that declares one,
//! and merges it into the platform's settings file without touching the user's
//! own entries.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::common::log::Logger;
use crate::config::platform::PlatformConfig;

use super::stophook::STOP_HOOK_COMMAND;
use super::{Policy, ScaffoldError};

/// Marks entries this module owns. Every design-lint command names this script.
const HOOK_COMMAND_MARKER: &str = "design-lint.";

/// First line of a Codewhale hooks file written by plaesy. Only files that start
/// with it are overwritten.
const CODEWHALE_OWNED_MARKER: &str = "# managed by plaesy";

/// Hook entries keyed by event name, in a stable order.
type Hooks = BTreeMap<String, Vec<Value>>;

const DESIGN_LINT_BASH: &str = r#"#!/bin/sh
# plaesy design-lint: advisory design check after a file edit. Never blocks; always exits 0.
# Reads the hook JSON on stdin. Lints the files the edit names. When the payload names
# none, it lints the frontend files git reports as changed. Needs the plaesy binary at the
# project root or on PATH; without it the hook does nothing.
root=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
input=$(cat)
named=$(printf '%s' "$input" | grep -oE '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' | sed -E 's/.*:[[:space:]]*"(.*)"$/\1/')
patched=$(printf '%s' "$input" | grep -oE '\*\*\* (Add|Update) File: [^\\"]+' | sed -E 's/^\*\*\* (Add|Update) File: //')
files=$(printf '%s\n%s\n' "$named" "$patched" | sed 's/\\\\/\//g' | sed '/^$/d')
if [ -z "$files" ]; then
  files=$(git -C "$root" status --porcelain 2>/dev/null | sed -E 's/^...//')
fi
bin="$root/plaesy"
[ -x "$bin" ] || bin="$root/plaesy.exe"
[ -x "$bin" ] || bin=$(command -v plaesy 2>/dev/null)
[ -n "$bin" ] || exit 0
printf '%s\n' "$files" | sort -u | while IFS= read -r f; do
  case "$f" in *.html|*.htm|*.css|*.scss|*.jsx|*.tsx|*.vue|*.svelte) ;; *) continue ;; esac
  p="$f"
  case "$p" in /*|?:*) ;; *) p="$root/$f" ;; esac
  [ -f "$p" ] || continue
  "$bin" validate design "$p" 2>&1 | grep -E '^(FAIL|WARN) '
done
exit 0
"#;

const DESIGN_LINT_POWERSHELL: &str = r#"# plaesy design-lint: advisory design check after a file edit. Never blocks; always exits 0.
$ErrorActionPreference = 'SilentlyContinue'
$root = git rev-parse --show-toplevel 2>$null
if (-not $root) { $root = (Get-Location).Path }
$payload = [Console]::In.ReadToEnd()
$named = [regex]::Matches($payload, '"file_path"\s*:\s*"([^"]*)"') | ForEach-Object { $_.Groups[1].Value }
$patched = [regex]::Matches($payload, '\*\*\* (?:Add|Update) File: ([^\\"]+)') | ForEach-Object { $_.Groups[1].Value }
$files = @($named) + @($patched) | ForEach-Object { $_ -replace '\\\\', '/' } | Where-Object { $_ }
if (-not $files) { $files = git -C $root status --porcelain | ForEach-Object { $_.Substring(3) } }
$bin = Join-Path $root 'plaesy.exe'
if (-not (Test-Path $bin)) { $bin = Join-Path $root 'plaesy' }
if (-not (Test-Path $bin)) {
  $cmd = Get-Command plaesy -ErrorAction SilentlyContinue
  if (-not $cmd) { exit 0 }
  $bin = $cmd.Source
}
foreach ($f in ($files | Sort-Object -Unique)) {
  if ($f -notmatch '\.(html|htm|css|scss|jsx|tsx|vue|svelte)$') { continue }
  $p = if ([System.IO.Path]::IsPathRooted($f)) { $f } else { Join-Path $root $f }
  if (-not (Test-Path $p -PathType Leaf)) { continue }
  & $bin validate design $p 2>&1 | Select-String -Pattern '^(FAIL|WARN) '
}
exit 0
"#;

/// Installs the design-lint hook for `platform`, if its `platform.json` entry names
/// a hooks manifest and a hooks format. Scripts are written first, then the manifest.
///
/// # Errors
///
/// [`ScaffoldError::Config`] for an unknown hooks format, or I/O failures.
pub fn install_platform_hooks(
    target_dir: &Path,
    platform: &str,
    cfg: &PlatformConfig,
    policy: &Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let Some(entry) = cfg.platforms.get(platform) else {
        return Ok(());
    };
    let (Some(manifest), Some(format)) = (
        entry.mapping.get("hooks"),
        entry.mapping.get("hooks_format"),
    ) else {
        return Ok(());
    };
    if manifest.is_empty() || format.is_empty() {
        return Ok(());
    }

    let scripts_rel = format!("{}/hooks", cfg.plaesy.base_directory);
    let scripts_dir = target_dir.join(&scripts_rel);
    write_hook_script(
        &scripts_dir.join("design-lint.sh"),
        DESIGN_LINT_BASH,
        policy,
    )?;
    write_hook_script(
        &scripts_dir.join("design-lint.ps1"),
        DESIGN_LINT_POWERSHELL,
        policy,
    )?;

    let sh_command = |prefix: &str| format!("sh \"{prefix}/{scripts_rel}/design-lint.sh\"");
    let ps_command = format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -File {scripts_rel}/design-lint.ps1"
    );
    let dst = target_dir.join(manifest);
    let sh_script = format!("sh {scripts_rel}/design-lint.sh");

    match format.as_str() {
        "claude" => {
            let mut desired = claude_post_tool(&sh_command("$CLAUDE_PROJECT_DIR"));
            desired.extend(claude_stop());
            merge_hook_manifest(&dst, "claude", &desired, policy, log)?;
        }
        "codex" => {
            let desired =
                codex_post_tool(&sh_command("$(git rev-parse --show-toplevel)"), &ps_command);
            merge_hook_manifest(&dst, "codex", &desired, policy, log)?;
        }
        "gemini" => {
            let desired = gemini_after_tool(&sh_command("$GEMINI_PROJECT_DIR"));
            merge_hook_manifest(&dst, "gemini", &desired, policy, log)?;
        }
        "windsurf" => {
            let desired = windsurf_post_write(&sh_script, &ps_command);
            merge_hook_manifest(&dst, "windsurf", &desired, policy, log)?;
        }
        "codewhale" => write_codewhale_hooks(&dst, &sh_script, policy, log)?,
        "qoder" => {
            let desired = claude_post_tool(&sh_command("${QODER_PROJECT_DIR}"));
            merge_hook_manifest(&dst, "qoder", &desired, policy, log)?;
        }
        "cursor" => {
            let desired = cursor_after_edit(&sh_script);
            merge_hook_manifest(&dst, "cursor", &desired, policy, log)?;
        }
        "copilot" => {
            write_copilot_hooks(
                &dst,
                &format!("sh ./{scripts_rel}/design-lint.sh"),
                &ps_command,
                policy,
            )?;
        }
        other => {
            return Err(ScaffoldError::Config(format!(
                "unknown hooks_format {other:?} for platform {platform}"
            )))
        }
    }
    log.success(&format!(
        "  ✓ design-lint hook for {} → {manifest}",
        cfg.display_name(platform)
    ));
    Ok(())
}

/// Writes one script. An existing script is replaced only when the policy overwrites.
fn write_hook_script(path: &Path, content: &str, policy: &Policy) -> Result<(), ScaffoldError> {
    if path.exists() && !policy.overwrite {
        return Ok(());
    }
    if policy.dry_run {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ScaffoldError::io(format!("creating {}", parent.display()), e))?;
    }
    fs::write(path, content)
        .map_err(|e| ScaffoldError::io(format!("writing {}", path.display()), e))?;
    set_executable(path);
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) {}

fn claude_post_tool(command: &str) -> Hooks {
    BTreeMap::from([(
        "PostToolUse".to_owned(),
        vec![json!({
            "matcher": "Edit|Write",
            "hooks": [{ "type": "command", "command": command, "timeout": 20, "statusMessage": "Design lint" }],
        })],
    )])
}

/// The Stop entry of the loop. Claude's Stop event takes no matcher.
fn claude_stop() -> Hooks {
    BTreeMap::from([(
        "Stop".to_owned(),
        vec![json!({
            "hooks": [{ "type": "command", "command": STOP_HOOK_COMMAND, "timeout": 30 }],
        })],
    )])
}

fn codex_post_tool(sh_command: &str, ps_command: &str) -> Hooks {
    BTreeMap::from([(
        "PostToolUse".to_owned(),
        vec![json!({
            "matcher": "apply_patch|Edit|Write",
            "hooks": [{
                "type": "command",
                "command": sh_command,
                "commandWindows": ps_command,
                "timeout": 20,
                "statusMessage": "Design lint",
            }],
        })],
    )])
}

fn gemini_after_tool(command: &str) -> Hooks {
    BTreeMap::from([(
        "AfterTool".to_owned(),
        vec![json!({
            "matcher": "write_file|replace",
            "hooks": [{ "name": "plaesy-design-lint", "type": "command", "command": command, "timeout": 20000 }],
        })],
    )])
}

/// Windsurf's shape: event keys map to arrays of `{command, powershell, show_output}`.
fn windsurf_post_write(sh_command: &str, ps_command: &str) -> Hooks {
    BTreeMap::from([(
        "post_write_code".to_owned(),
        vec![json!({ "command": sh_command, "powershell": ps_command, "show_output": true })],
    )])
}

fn cursor_after_edit(command: &str) -> Hooks {
    BTreeMap::from([(
        "afterFileEdit".to_owned(),
        vec![json!({ "command": command, "matcher": "Write", "timeout": 20 })],
    )])
}

/// Merges `desired` into a JSON settings file. Entries plaesy owned on an earlier
/// install are dropped and the current ones appended. Every other key and entry is
/// kept. A file that is not valid JSON is left alone.
fn merge_hook_manifest(
    path: &Path,
    format: &str,
    desired: &Hooks,
    policy: &Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    let mut root: Map<String, Value> = match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => map,
            _ => {
                // A broken user file is reported and never overwritten.
                log.warning(&format!(
                    "  ⊘ {} is not valid JSON; hook not installed, fix the file and run reload",
                    path.display()
                ));
                return Ok(());
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(ScaffoldError::io(format!("reading {}", path.display()), e)),
    };
    if format == "cursor" {
        root.entry("version").or_insert(json!(1));
    }

    if policy.dry_run {
        return Ok(());
    }
    let mut hooks = match root.remove("hooks") {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    for (event, entries) in desired {
        let mut kept: Vec<Value> = match hooks.remove(event) {
            Some(Value::Array(existing)) => existing
                .into_iter()
                .filter(|e| !is_plaesy_entry(e))
                .collect(),
            _ => Vec::new(),
        };
        kept.extend(entries.iter().cloned());
        hooks.insert(event.clone(), Value::Array(kept));
    }
    root.insert("hooks".to_owned(), Value::Object(hooks));
    write_json(path, &Value::Object(root))
}

/// Copilot's hook file is plaesy's alone, so it is rewritten whole.
fn write_copilot_hooks(
    path: &Path,
    bash: &str,
    powershell: &str,
    policy: &Policy,
) -> Result<(), ScaffoldError> {
    if policy.dry_run {
        return Ok(());
    }
    let doc = json!({
        "version": 1,
        "hooks": {
            "postToolUse": [{
                "type": "command",
                "bash": bash,
                "powershell": powershell,
                "cwd": ".",
                "timeoutSec": 20,
            }],
        },
    });
    write_json(path, &doc)
}

/// Writes Codewhale's project hook file. Codewhale runs project hooks only after the
/// user approves the file. `tool_call_after` gives no path, so the script uses git's
/// changed files. A file without the owned marker belongs to the user and is kept.
fn write_codewhale_hooks(
    path: &Path,
    command: &str,
    policy: &Policy,
    log: &Logger,
) -> Result<(), ScaffoldError> {
    if let Ok(existing) = fs::read_to_string(path) {
        if !existing.starts_with(CODEWHALE_OWNED_MARKER) {
            log.warning(&format!(
                "  ⊘ {} was not written by plaesy; hook not installed. Add the hook by hand or remove the file and run reload",
                path.display()
            ));
            return Ok(());
        }
    }
    if policy.dry_run {
        return Ok(());
    }
    let body = format!(
        "{CODEWHALE_OWNED_MARKER}: design-lint hook. Regenerated by `plaesy init` and `plaesy reload`.\n\
         [[hooks]]\n\
         event = \"tool_call_after\"\n\
         name = \"plaesy-design-lint\"\n\
         command = {command:?}\n\
         timeout_secs = 20\n\
         continue_on_error = true\n"
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ScaffoldError::io(format!("creating {}", parent.display()), e))?;
    }
    fs::write(path, body).map_err(|e| ScaffoldError::io(format!("writing {}", path.display()), e))
}

/// Reports whether an entry is one plaesy owns: the design-lint hook or the loop Stop hook.
fn is_plaesy_entry(entry: &Value) -> bool {
    let text = entry.to_string();
    text.contains(HOOK_COMMAND_MARKER) || text.contains(STOP_HOOK_COMMAND)
}

fn write_json(path: &Path, doc: &Value) -> Result<(), ScaffoldError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ScaffoldError::io(format!("creating {}", parent.display()), e))?;
    }
    let mut text = serde_json::to_string_pretty(doc)
        .map_err(|e| ScaffoldError::Config(format!("encoding {}: {e}", path.display())))?;
    text.push('\n');
    fs::write(path, text).map_err(|e| ScaffoldError::io(format!("writing {}", path.display()), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_user_entries_and_replaces_plaesy_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"hooks":{"PostToolUse":[
                {"matcher":"Bash","hooks":[{"type":"command","command":"my-own"}]},
                {"matcher":"Edit|Write","hooks":[{"type":"command","command":"sh x/design-lint.sh"}]}
            ]},"theme":"dark"}"#,
        )
        .unwrap();

        merge_hook_manifest(
            &path,
            "claude",
            &claude_post_tool("sh new/design-lint.sh"),
            &Policy::default(),
            &Logger::new("t"),
        )
        .unwrap();

        let value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let entries = value["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(
            entries.len(),
            2,
            "user Bash entry plus the new plaesy entry"
        );
        assert_eq!(entries[0]["hooks"][0]["command"], "my-own");
        assert_eq!(value["theme"], "dark");
    }

    #[test]
    fn invalid_settings_file_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ broken").unwrap();
        merge_hook_manifest(
            &path,
            "claude",
            &claude_stop(),
            &Policy::default(),
            &Logger::new("t"),
        )
        .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ broken");
    }

    #[test]
    fn codewhale_file_without_marker_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hooks.toml");
        fs::write(&path, "user content").unwrap();
        write_codewhale_hooks(&path, "sh x", &Policy::default(), &Logger::new("t")).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "user content");
    }
}
