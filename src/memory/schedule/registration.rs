//! Per-user operating-system registration for scheduled memory maintenance.

use crate::memory::{config::MemoryConfig, MemoryError, Result};
use std::path::{Path, PathBuf};

#[cfg(windows)]
const TASK_NAME: &str = "Plaesy Memory Schedule";
#[cfg(target_os = "linux")]
const UNIT_NAME: &str = "plaesy-memory";
#[cfg(target_os = "macos")]
const LAUNCH_AGENT: &str = "com.plaesy.memory-schedule";

fn config_file() -> Result<PathBuf> {
    let root = dirs::config_dir().ok_or_else(|| {
        MemoryError::ConfigError("could not resolve the user configuration directory".into())
    })?;
    Ok(root.join("plaesy").join("schedule").join("memory.toml"))
}

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        MemoryError::ConfigError(format!(
            "configuration path has no parent: {}",
            path.display()
        ))
    })?;
    std::fs::create_dir_all(parent).map_err(|source| MemoryError::IoError {
        path: parent.display().to_string(),
        source,
    })?;
    let temporary = path.with_extension("toml.tmp");
    std::fs::write(&temporary, contents).map_err(|source| MemoryError::IoError {
        path: temporary.display().to_string(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).map_err(
            |source| MemoryError::IoError {
                path: temporary.display().to_string(),
                source,
            },
        )?;
    }
    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path).map_err(|source| MemoryError::IoError {
            path: path.display().to_string(),
            source,
        })?;
    }
    std::fs::rename(&temporary, path).map_err(|source| MemoryError::IoError {
        path: path.display().to_string(),
        source,
    })
}

/// Register a user-level task that invokes `schedule once` every 30 minutes.
pub fn install(config: &MemoryConfig) -> Result<()> {
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    let executable = std::env::current_exe().map_err(|source| MemoryError::IoError {
        path: "current executable".into(),
        source,
    })?;
    let snapshot = config_file()?;
    let mut snapshot_config = config.clone();
    let vault =
        std::fs::canonicalize(&config.watch_dir).map_err(|source| MemoryError::IoError {
            path: config.watch_dir.display().to_string(),
            source,
        })?;
    snapshot_config.watch_dir = vault.clone();
    if snapshot_config.database.db_path.to_string_lossy() == ":memory:" {
        return Err(MemoryError::ConfigError(
            "cannot install a persistent schedule with an in-memory database; configure a file-backed database first".into(),
        ));
    }
    snapshot_config.database.db_path = absolute_path(&snapshot_config.database.db_path)?;
    if let Some(model_dir) = snapshot_config.embeddings.model_dir.clone() {
        snapshot_config.embeddings.model_dir = Some(absolute_path(&model_dir)?);
    }
    let contents = toml::to_string_pretty(&snapshot_config).map_err(|error| {
        MemoryError::ConfigError(format!("cannot save schedule config: {error}"))
    })?;
    atomic_write(&snapshot, &contents)?;

    #[cfg(windows)]
    install_windows(
        &executable,
        &snapshot,
        &vault,
        &snapshot_config.database.db_path,
    )?;
    #[cfg(target_os = "linux")]
    install_linux(
        &executable,
        &snapshot,
        &vault,
        &snapshot_config.database.db_path,
    )?;
    #[cfg(target_os = "macos")]
    install_macos(
        &executable,
        &snapshot,
        &vault,
        &snapshot_config.database.db_path,
    )?;
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    return Err(MemoryError::ConfigError(
        "automatic scheduling is supported on Windows, Linux (systemd user), and macOS".into(),
    ));

    println!("Installed per-user Plaesy memory schedule (every 30 minutes).");
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| MemoryError::IoError {
                path: "current working directory".into(),
                source,
            })?
            .join(path)
    };
    Ok(std::fs::canonicalize(&absolute).unwrap_or(absolute))
}

/// Remove the per-user operating-system registration and its generated config snapshot.
pub fn uninstall() -> Result<()> {
    #[cfg(windows)]
    uninstall_windows()?;
    #[cfg(target_os = "linux")]
    uninstall_linux()?;
    #[cfg(target_os = "macos")]
    uninstall_macos()?;
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    return Err(MemoryError::ConfigError(
        "automatic scheduling is supported on Windows, Linux (systemd user), and macOS".into(),
    ));

    match config_file() {
        Ok(path) if path.exists() => {
            std::fs::remove_file(&path).map_err(|source| MemoryError::IoError {
                path: path.display().to_string(),
                source,
            })?
        }
        _ => {}
    }
    println!("Removed per-user Plaesy memory schedule.");
    Ok(())
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .map_err(|source| MemoryError::IoError {
            path: program.into(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(MemoryError::ConfigError(format!(
            "{} failed with status {}",
            program, status
        )))
    }
}

#[cfg(windows)]
fn install_windows(exe: &Path, config: &Path, vault: &Path, db: &Path) -> Result<()> {
    let invocation = format!(
        "{} memory --vault {} --config {} --db {} schedule once",
        quote_windows(&exe.to_string_lossy()),
        quote_windows(&vault.to_string_lossy()),
        quote_windows(&config.to_string_lossy()),
        quote_windows(&db.to_string_lossy())
    );
    run(
        "schtasks.exe",
        &[
            "/Create",
            "/F",
            "/TN",
            TASK_NAME,
            "/TR",
            &invocation,
            "/SC",
            "MINUTE",
            "/MO",
            "30",
            "/IT",
        ],
    )?;
    run("schtasks.exe", &["/Run", "/TN", TASK_NAME])
}

#[cfg(windows)]
fn uninstall_windows() -> Result<()> {
    let exists = std::process::Command::new("schtasks.exe")
        .args(["/Query", "/TN", TASK_NAME])
        .status()
        .is_ok_and(|status| status.success());
    if exists {
        run("schtasks.exe", &["/Delete", "/F", "/TN", TASK_NAME])?;
    }
    Ok(())
}

#[cfg(windows)]
fn quote_windows(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

#[cfg(target_os = "linux")]
fn install_linux(exe: &Path, config: &Path, vault: &Path, db: &Path) -> Result<()> {
    let root = dirs::config_dir()
        .ok_or_else(|| {
            MemoryError::ConfigError("could not resolve the user configuration directory".into())
        })?
        .join("systemd")
        .join("user");
    std::fs::create_dir_all(&root).map_err(|source| MemoryError::IoError {
        path: root.display().to_string(),
        source,
    })?;
    let service = format!(
        "[Unit]\nDescription=Run due Plaesy memory jobs\n\n[Service]\nType=oneshot\nExecStart={} memory --vault {} --config {} --db {} schedule once\n",
        systemd_arg(&exe.to_string_lossy()),
        systemd_arg(&vault.to_string_lossy()),
        systemd_arg(&config.to_string_lossy()),
        systemd_arg(&db.to_string_lossy())
    );
    let timer = format!(
        "[Unit]\nDescription=Schedule Plaesy memory jobs\n\n[Timer]\nOnCalendar=*-*-* *:0/30:00\nPersistent=true\nUnit={UNIT_NAME}.service\n\n[Install]\nWantedBy=timers.target\n"
    );
    atomic_write(&root.join(format!("{UNIT_NAME}.service")), &service)?;
    atomic_write(&root.join(format!("{UNIT_NAME}.timer")), &timer)?;
    run("systemctl", &["--user", "daemon-reload"])?;
    run(
        "systemctl",
        &["--user", "enable", "--now", &format!("{UNIT_NAME}.timer")],
    )
}

#[cfg(target_os = "linux")]
fn uninstall_linux() -> Result<()> {
    let timer = format!("{UNIT_NAME}.timer");
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "--now", &timer])
        .status();
    let root = dirs::config_dir()
        .ok_or_else(|| {
            MemoryError::ConfigError("could not resolve the user configuration directory".into())
        })?
        .join("systemd")
        .join("user");
    for name in [format!("{UNIT_NAME}.service"), timer] {
        let path = root.join(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|source| MemoryError::IoError {
                path: path.display().to_string(),
                source,
            })?;
        }
    }
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    Ok(())
}

#[cfg(target_os = "linux")]
fn systemd_arg(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}

#[cfg(target_os = "macos")]
fn install_macos(exe: &Path, config: &Path, vault: &Path, db: &Path) -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| MemoryError::ConfigError("could not resolve home directory".into()))?;
    let agents = home.join("Library").join("LaunchAgents");
    let logs = home.join("Library").join("Logs");
    std::fs::create_dir_all(&agents).map_err(|source| MemoryError::IoError {
        path: agents.display().to_string(),
        source,
    })?;
    std::fs::create_dir_all(&logs).map_err(|source| MemoryError::IoError {
        path: logs.display().to_string(),
        source,
    })?;
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>{LAUNCH_AGENT}</string>\n<key>ProgramArguments</key><array><string>{}</string><string>memory</string><string>--vault</string><string>{}</string><string>--config</string><string>{}</string><string>--db</string><string>{}</string><string>schedule</string><string>once</string></array>\n<key>StartInterval</key><integer>1800</integer>\n<key>StandardOutPath</key><string>{}</string>\n<key>StandardErrorPath</key><string>{}</string>\n</dict></plist>\n",
        xml(&exe.to_string_lossy()), xml(&vault.to_string_lossy()),
        xml(&config.to_string_lossy()), xml(&db.to_string_lossy()),
        xml(&logs.join("plaesy-memory-schedule.log").to_string_lossy()),
        xml(&logs.join("plaesy-memory-schedule.log").to_string_lossy())
    );
    let path = agents.join(format!("{LAUNCH_AGENT}.plist"));
    atomic_write(&path, &plist)?;
    let domain = format!("gui/{}", unsafe { libc::getuid() });
    let label = format!("{domain}/{LAUNCH_AGENT}");
    let _ = std::process::Command::new("launchctl")
        .args(["bootout", &label])
        .status();
    run(
        "launchctl",
        &[
            "bootstrap",
            &domain,
            path.to_str().ok_or_else(|| {
                MemoryError::ConfigError("LaunchAgent path is not valid UTF-8".into())
            })?,
        ],
    )
}

#[cfg(target_os = "macos")]
fn uninstall_macos() -> Result<()> {
    let domain = format!("gui/{}", unsafe { libc::getuid() });
    let label = format!("{domain}/{LAUNCH_AGENT}");
    let _ = std::process::Command::new("launchctl")
        .args(["bootout", &label])
        .status();
    let home = dirs::home_dir()
        .ok_or_else(|| MemoryError::ConfigError("could not resolve home directory".into()))?;
    let path = home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCH_AGENT}.plist"));
    if path.exists() {
        std::fs::remove_file(&path).map_err(|source| MemoryError::IoError {
            path: path.display().to_string(),
            source,
        })?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
