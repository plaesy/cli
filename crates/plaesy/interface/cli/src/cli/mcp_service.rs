//! Autostart for the shared memory MCP server as a per-user systemd service.
//!
//! `start` writes a user unit that runs `plaesy mcp memory serve` and keeps it running
//! (`Restart=on-failure`), so watchers that depend on the memory server are available after login
//! without a separate process manager. Linux only; other platforms return a clear error.

use std::path::{Path, PathBuf};

type Res<T = ()> = Result<T, Box<dyn std::error::Error>>;

const UNIT_NAME: &str = "plaesy-memory-mcp";
const DEFAULT_PORT: u16 = 9123;

fn unit_path() -> Res<PathBuf> {
    let base = dirs::config_dir().ok_or("could not resolve the user configuration directory")?;
    Ok(base
        .join("systemd")
        .join("user")
        .join(format!("{UNIT_NAME}.service")))
}

fn settings_path() -> Res<PathBuf> {
    let base = dirs::config_dir().ok_or("could not resolve the user configuration directory")?;
    Ok(base.join("plaesy").join("memory-mcp").join("settings.json"))
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Settings {
    vault: PathBuf,
    port: u16,
}

fn read_settings() -> Res<Option<Settings>> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let settings =
        serde_json::from_str(&text).map_err(|e| format!("invalid {}: {e}", path.display()))?;
    Ok(Some(settings))
}

fn write_settings(settings: &Settings) -> Res<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("cannot encode settings: {e}"))?;
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(())
}

/// Quote one `ExecStart=` argument for systemd (double quotes, escape `\` and `"`, `%` as `%%`).
fn quote_systemd(arg: &str) -> String {
    let escaped = arg
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

fn render_unit(exe: &Path, vault: &Path, port: u16) -> String {
    let exec = [
        quote_systemd(&exe.to_string_lossy()),
        "mcp".into(),
        "memory".into(),
        "serve".into(),
        "--vault".into(),
        quote_systemd(&vault.to_string_lossy()),
        "--port".into(),
        port.to_string(),
    ]
    .join(" ");
    format!(
        "[Unit]\n\
         Description=Plaesy shared memory MCP server\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         EnvironmentFile=-%h/.config/plaesy/memory-mcp/env\n\
         ExecStart={exec}\n\
         Restart=on-failure\n\
         RestartSec=30\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

fn systemctl(args: &[&str]) -> Res<String> {
    let output = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|e| format!("cannot run systemctl: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            stderr.trim()
        )
        .into())
    }
}

/// Query-style systemctl call: `is-active`/`is-enabled` exit non-zero for valid
/// states such as "inactive", so the printed state is returned regardless of exit code.
#[cfg(target_os = "linux")]
fn systemctl_state(args: &[&str]) -> String {
    match std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
    {
        Ok(output) => {
            let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if out.is_empty() {
                "unknown".to_string()
            } else {
                out
            }
        }
        Err(e) => format!("unavailable ({e})"),
    }
}

#[cfg(target_os = "linux")]
pub fn start(vault: Option<&Path>, port: Option<u16>) -> Res {
    let previous = read_settings()?;
    let vault = match (vault, &previous) {
        (Some(v), _) => v.to_path_buf(),
        (None, Some(s)) => s.vault.clone(),
        (None, None) => return Err("first start needs --vault <dir>".into()),
    };
    let vault = std::fs::canonicalize(&vault)
        .map_err(|e| format!("vault {} is not accessible: {e}", vault.display()))?;
    let port = port
        .or(previous.as_ref().map(|s| s.port))
        .unwrap_or(DEFAULT_PORT);
    let exe = std::env::current_exe().map_err(|e| format!("cannot resolve executable: {e}"))?;

    let unit = unit_path()?;
    if let Some(parent) = unit.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    std::fs::write(&unit, render_unit(&exe, &vault, port))
        .map_err(|e| format!("cannot write {}: {e}", unit.display()))?;
    write_settings(&Settings {
        vault: vault.clone(),
        port,
    })?;

    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", &format!("{UNIT_NAME}.service")])?;
    println!(
        "Started {UNIT_NAME} (vault {}, port {port}). Check with `plaesy mcp memory status`.",
        vault.display()
    );
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn stop() -> Res {
    systemctl(&["stop", &format!("{UNIT_NAME}.service")])?;
    println!("Stopped {UNIT_NAME}; autostart registration kept.");
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn status() -> Res {
    let unit = unit_path()?;
    if !unit.exists() {
        println!("{UNIT_NAME}: not installed. Run `plaesy mcp memory start --vault <dir>`.");
        return Ok(());
    }
    let active = systemctl_state(&["is-active", &format!("{UNIT_NAME}.service")]);
    let enabled = systemctl_state(&["is-enabled", &format!("{UNIT_NAME}.service")]);
    println!("{UNIT_NAME}: active={active} enabled={enabled}");
    if let Some(s) = read_settings()? {
        println!("vault={} port={}", s.vault.display(), s.port);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn uninstall() -> Res {
    let _ = systemctl(&["disable", "--now", &format!("{UNIT_NAME}.service")]);
    let unit = unit_path()?;
    if unit.exists() {
        std::fs::remove_file(&unit)
            .map_err(|e| format!("cannot remove {}: {e}", unit.display()))?;
    }
    let settings = settings_path()?;
    if settings.exists() {
        std::fs::remove_file(&settings)
            .map_err(|e| format!("cannot remove {}: {e}", settings.display()))?;
    }
    systemctl(&["daemon-reload"])?;
    println!("Removed {UNIT_NAME}.");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn start(_vault: Option<&Path>, _port: Option<u16>) -> Res {
    Err("background memory MCP service is supported on Linux (systemd user) only".into())
}

#[cfg(not(target_os = "linux"))]
pub fn stop() -> Res {
    Err("background memory MCP service is supported on Linux (systemd user) only".into())
}

#[cfg(not(target_os = "linux"))]
pub fn status() -> Res {
    Err("background memory MCP service is supported on Linux (systemd user) only".into())
}

#[cfg(not(target_os = "linux"))]
pub fn uninstall() -> Res {
    Err("background memory MCP service is supported on Linux (systemd user) only".into())
}

#[cfg(test)]
#[path = "mcp_service_test.rs"]
mod mcp_service_test;
