//! Per-user scheduling for persisted `/goal` chat sessions.
//!
//! A schedule invokes `plaesy chat --resume` through a small run-once wrapper. The wrapper owns a
//! per-schedule lock, so overlapping OS timer firings do not create concurrent model runs. The
//! resumed chat writes back to the same session file, making each firing observe the previous run.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use clap::{Args as ClapArgs, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::chat_history;
use super::CliResult;

const LOCK_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    pub command: CommandKind,
}

#[derive(Debug, Subcommand)]
pub enum CommandKind {
    /// Install a per-user OS timer for one saved chat session.
    Install {
        /// Saved session file name under `<path>/.plaesy/chats/`.
        session: String,
        /// Interval such as `15m`, `1h`, or `1d`.
        #[arg(long, default_value = "30m")]
        every: String,
        /// Project root containing `.plaesy/chats/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
        /// Preserve the chat's ability to mutate repository state.
        #[arg(long)]
        allow_writes: bool,
        /// Require persisted approvals for mutating tools.
        #[arg(long)]
        supervised: bool,
    },
    /// Execute one due goal run. Used by the installed OS timer.
    #[command(hide = true)]
    RunOnce {
        /// Absolute schedule specification written by `install`.
        #[arg(long)]
        schedule: PathBuf,
    },
    /// Remove one installed goal timer and its specification.
    Uninstall {
        /// Absolute schedule specification written by `install`.
        #[arg(long)]
        schedule: PathBuf,
    },
    /// List installed goal timers for a project.
    List {
        /// Project root containing `.plaesy/goal-schedules/`.
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ScheduleSpec {
    id: String,
    session: String,
    root: PathBuf,
    every_seconds: u64,
    allow_writes: bool,
    supervised: bool,
}

pub fn run(args: &Args) -> CliResult {
    match &args.command {
        CommandKind::Install {
            session,
            every,
            path,
            allow_writes,
            supervised,
        } => install(session, every, path, *allow_writes, *supervised),
        CommandKind::RunOnce { schedule } => run_once(schedule),
        CommandKind::Uninstall { schedule } => uninstall(schedule),
        CommandKind::List { path } => list(path),
    }
}

fn install(
    session: &str,
    every: &str,
    path: &Path,
    allow_writes: bool,
    supervised: bool,
) -> CliResult {
    chat_history::check_name(session)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let root = fs::canonicalize(path)?;
    let session_path = root.join(".plaesy").join("chats").join(session);
    if !session_path.is_file() {
        return Err(format!("saved session does not exist: {}", session_path.display()).into());
    }
    let every_seconds =
        parse_interval(every).map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let id = schedule_id(&root, session);
    let spec = ScheduleSpec {
        id: id.clone(),
        session: session.to_owned(),
        root: root.clone(),
        every_seconds,
        allow_writes,
        supervised,
    };
    let dir = root.join(".plaesy").join("goal-schedules");
    fs::create_dir_all(&dir)?;
    let schedule = dir.join(format!("goal-{id}.json"));
    write_json(&schedule, &spec)?;
    if let Err(err) = register(&schedule, &spec) {
        let _ = fs::remove_file(&schedule);
        return Err(err.into());
    }
    println!("installed goal schedule {} every {}", id, every);
    Ok(())
}

fn run_once(schedule: &Path) -> CliResult {
    let spec: ScheduleSpec = read_json(schedule)?;
    let _lock = match RunLock::acquire(&schedule.with_extension("lock"))? {
        Some(lock) => lock,
        None => {
            println!(
                "goal schedule {} skipped: previous run is still active",
                spec.id
            );
            return Ok(());
        }
    };
    let executable = std::env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("chat")
        .arg("--path")
        .arg(&spec.root)
        .arg("--resume")
        .arg(&spec.session)
        .stdin(Stdio::null());
    if spec.allow_writes {
        command.arg("--allow-writes");
    }
    if spec.supervised {
        command.arg("--supervised");
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("goal schedule {} exited with {status}", spec.id).into())
    }
}

fn uninstall(schedule: &Path) -> CliResult {
    let spec: ScheduleSpec = read_json(schedule)?;
    unregister(&spec)?;
    if schedule.exists() {
        fs::remove_file(schedule)?;
    }
    println!("uninstalled goal schedule {}", spec.id);
    Ok(())
}

fn list(path: &Path) -> CliResult {
    let root = fs::canonicalize(path)?;
    let dir = root.join(".plaesy").join("goal-schedules");
    if !dir.is_dir() {
        return Ok(());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    for path in paths {
        let spec: ScheduleSpec = read_json(&path)?;
        println!(
            "{}\t{}\tevery {}s\twrites={} supervised={}",
            spec.id, spec.session, spec.every_seconds, spec.allow_writes, spec.supervised
        );
    }
    Ok(())
}

fn parse_interval(value: &str) -> Result<u64, String> {
    let value = value.trim();
    let (number, multiplier) = value.split_at(value.len().saturating_sub(1));
    let multiplier = match multiplier {
        "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => return Err(format!("invalid interval {value:?}; use Ns, Nm, Nh, or Nd")),
    };
    let number = number
        .parse::<u64>()
        .map_err(|_| format!("invalid interval {value:?}"))?;
    if number == 0 {
        return Err("interval must be greater than zero".into());
    }
    number
        .checked_mul(multiplier)
        .ok_or_else(|| "interval is too large".to_owned())
}

fn schedule_id(root: &Path, session: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(root.to_string_lossy().as_bytes());
    digest.update([0]);
    digest.update(session.as_bytes());
    let short: String = digest
        .finalize()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    short
}

fn write_json(path: &Path, value: &ScheduleSpec) -> io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(value).map_err(io::Error::other)?,
    )?;
    // Windows does not replace an existing destination during rename.
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)
}

fn read_json(path: &Path) -> io::Result<ScheduleSpec> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

struct RunLock {
    path: PathBuf,
}

impl RunLock {
    fn acquire(path: &Path) -> io::Result<Option<Self>> {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(_) => Ok(Some(Self {
                path: path.to_owned(),
            })),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let stale = fs::metadata(path)
                    .and_then(|meta| meta.modified())
                    .and_then(|modified| {
                        SystemTime::now()
                            .duration_since(modified)
                            .map_err(io::Error::other)
                    })
                    .is_ok_and(|age| age > LOCK_MAX_AGE);
                if stale {
                    let _ = fs::remove_file(path);
                    return Self::acquire(path);
                }
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn register(schedule: &Path, spec: &ScheduleSpec) -> Result<(), String> {
    #[cfg(windows)]
    {
        let task_name = task_name(spec);
        let command = scheduled_command(schedule);
        run_process(
            "schtasks.exe",
            &[
                "/Create",
                "/F",
                "/TN",
                &task_name,
                "/TR",
                &command,
                "/SC",
                "MINUTE",
                "/MO",
                &minutes(spec.every_seconds),
            ],
        )?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        register_linux(schedule, spec)
    }
    #[cfg(target_os = "macos")]
    {
        register_macos(schedule, spec)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = (schedule, spec);
        Err("goal scheduling is supported on Windows, Linux, and macOS".into())
    }
}

fn unregister(spec: &ScheduleSpec) -> Result<(), String> {
    #[cfg(windows)]
    {
        let task_name = task_name(spec);
        let _ = Command::new("schtasks.exe")
            .args(["/Delete", "/F", "/TN", &task_name])
            .status();
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        unregister_linux(spec)
    }
    #[cfg(target_os = "macos")]
    {
        unregister_macos(spec)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = spec;
        Err("goal scheduling is supported on Windows, Linux, and macOS".into())
    }
}

fn scheduled_command(schedule: &Path) -> String {
    format!(
        "{} chat-schedule run-once --schedule {}",
        quote_arg(
            &std::env::current_exe()
                .unwrap_or_default()
                .to_string_lossy()
        ),
        quote_arg(&schedule.to_string_lossy())
    )
}

fn run_process(program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("{program}: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{program} exited with {status}"))
}

#[cfg(windows)]
fn task_name(spec: &ScheduleSpec) -> String {
    format!("Plaesy Goal {}", spec.id)
}

#[cfg(windows)]
fn minutes(seconds: u64) -> String {
    seconds.div_ceil(60).max(1).to_string()
}

fn quote_arg(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

#[cfg(target_os = "linux")]
fn register_linux(schedule: &Path, spec: &ScheduleSpec) -> Result<(), String> {
    let root = dirs::config_dir()
        .ok_or_else(|| "could not resolve user configuration directory".to_owned())?
        .join("systemd")
        .join("user");
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let name = format!("plaesy-goal-{}", spec.id);
    let service = format!(
        "[Unit]\nDescription=Run Plaesy goal {}\n\n[Service]\nType=oneshot\nExecStart={}\n",
        spec.id,
        systemd_quote(&scheduled_command(schedule))
    );
    let timer = format!(
        "[Unit]\nDescription=Schedule Plaesy goal {}\n\n[Timer]\nOnUnitActiveSec={}s\nPersistent=true\nUnit={name}.service\n\n[Install]\nWantedBy=timers.target\n",
        spec.id, spec.every_seconds
    );
    fs::write(root.join(format!("{name}.service")), service).map_err(|error| error.to_string())?;
    fs::write(root.join(format!("{name}.timer")), timer).map_err(|error| error.to_string())?;
    run_process("systemctl", &["--user", "daemon-reload"])?;
    run_process(
        "systemctl",
        &["--user", "enable", "--now", &format!("{name}.timer")],
    )
}

#[cfg(target_os = "linux")]
fn unregister_linux(spec: &ScheduleSpec) -> Result<(), String> {
    let name = format!("plaesy-goal-{}", spec.id);
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", &format!("{name}.timer")])
        .status();
    let root = dirs::config_dir()
        .ok_or_else(|| "could not resolve user configuration directory".to_owned())?
        .join("systemd")
        .join("user");
    for suffix in ["service", "timer"] {
        let path = root.join(format!("{name}.{suffix}"));
        if path.exists() {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    Ok(())
}

#[cfg(target_os = "linux")]
fn systemd_quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}

#[cfg(target_os = "macos")]
extern "C" {
    /// POSIX `getuid`: the real user id of the calling process. Declared here so this crate does
    /// not need a `libc` dependency, which would change `Cargo.lock`.
    fn getuid() -> u32;
}

#[cfg(target_os = "macos")]
fn register_macos(schedule: &Path, spec: &ScheduleSpec) -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_owned())?;
    let agents = home.join("Library").join("LaunchAgents");
    fs::create_dir_all(&agents).map_err(|error| error.to_string())?;
    let label = format!("com.plaesy.goal-{}", spec.id);
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>{label}</string><key>ProgramArguments</key><array><string>{}</string><string>chat-schedule</string><string>run-once</string><string>--schedule</string><string>{}</string></array><key>StartInterval</key><integer>{}</integer></dict></plist>\n",
        xml(&std::env::current_exe().map_err(|error| error.to_string())?.to_string_lossy()),
        xml(&schedule.to_string_lossy()),
        spec.every_seconds
    );
    let path = agents.join(format!("{label}.plist"));
    fs::write(&path, plist).map_err(|error| error.to_string())?;
    let domain = format!("gui/{}", unsafe { getuid() });
    let qualified = format!("{domain}/{label}");
    let _ = Command::new("launchctl")
        .args(["bootout", &qualified])
        .status();
    run_process(
        "launchctl",
        &[
            "bootstrap",
            &domain,
            path.to_str()
                .ok_or_else(|| "invalid LaunchAgent path".to_owned())?,
        ],
    )
}

#[cfg(target_os = "macos")]
fn unregister_macos(spec: &ScheduleSpec) -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_owned())?;
    let label = format!("com.plaesy.goal-{}", spec.id);
    let domain = format!("gui/{}", unsafe { getuid() });
    let qualified = format!("{domain}/{label}");
    let _ = Command::new("launchctl")
        .args(["bootout", &qualified])
        .status();
    let path = home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{label}.plist"));
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::{parse_interval, schedule_id, ScheduleSpec};
    use std::path::Path;

    #[test]
    fn interval_parser_accepts_units_and_rejects_zero() {
        assert_eq!(parse_interval("15m"), Ok(900));
        assert_eq!(parse_interval("2h"), Ok(7_200));
        assert!(parse_interval("0s").is_err());
        assert!(parse_interval("ten").is_err());
    }

    #[test]
    fn schedule_id_is_stable_and_scoped_to_root_and_session() {
        let a = schedule_id(Path::new("C:/project"), "chat-1.jsonl");
        assert_eq!(a, schedule_id(Path::new("C:/project"), "chat-1.jsonl"));
        assert_ne!(a, schedule_id(Path::new("C:/other"), "chat-1.jsonl"));
        assert_ne!(a, schedule_id(Path::new("C:/project"), "chat-2.jsonl"));
    }

    #[test]
    fn schedule_spec_round_trips() {
        let spec = ScheduleSpec {
            id: "abc".into(),
            session: "chat-1.jsonl".into(),
            root: Path::new("C:/project").into(),
            every_seconds: 900,
            allow_writes: false,
            supervised: true,
        };
        let json = serde_json::to_string(&spec).expect("serialize");
        assert_eq!(
            serde_json::from_str::<ScheduleSpec>(&json).expect("parse"),
            spec
        );
    }
}
