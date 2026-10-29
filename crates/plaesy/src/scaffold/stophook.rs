//! The loop Stop hook: keeps a running autonomous loop from ending its turn early.
//!
//! The hook returns exit code 2 only while the run is live and no report has been
//! written. Claude Code treats exit 2 as "continue", and sends the message on
//! stderr back to the model.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Folder, under the project root, where a loop run writes its reports.
pub const LOOP_REPORT_DIR: &str = ".plaesy/loop";

/// How old the driver's heartbeat may be and still count as live.
pub const LOOP_HEARTBEAT_WINDOW_SECS: i64 = 10 * 60;

/// Report that marks a completed run.
pub const COMPLETION_REPORT_FILE: &str = "completion-report.md";
/// Report that marks a stopped run.
pub const STOP_REPORT_FILE: &str = "stop-report.md";

/// Command a Stop hook entry runs.
pub const STOP_HOOK_COMMAND: &str = "plaesy loop stop-hook";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HookInput {
    stop_hook_active: bool,
    background_tasks: Vec<serde_json::Value>,
    session_crons: Vec<serde_json::Value>,
}

/// The part of `autonomous_loop` in `.plaesy/state.json` that the hook reads.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LoopRun {
    pub status: String,
    pub driver_pid: i64,
    pub heartbeat_at: String,
}

impl LoopRun {
    /// Reports whether a driver is working on the run: the heartbeat parses, is
    /// within the window of `now`, and the recorded pid is alive.
    #[must_use]
    pub fn is_live(&self, now: DateTime<Utc>) -> bool {
        let Ok(beat) = DateTime::parse_from_rfc3339(&self.heartbeat_at) else {
            return false;
        };
        let age = (now - beat.with_timezone(&Utc)).num_seconds().abs();
        age <= LOOP_HEARTBEAT_WINDOW_SECS && pid_alive(self.driver_pid)
    }
}

/// Reads `autonomous_loop` from the state file. A missing or unreadable file yields
/// no status, so the hook does not block.
#[must_use]
pub fn read_loop_run(state_file: &Path) -> LoopRun {
    #[derive(Deserialize)]
    struct State {
        #[serde(default)]
        autonomous_loop: LoopRun,
    }
    fs::read_to_string(state_file)
        .ok()
        .and_then(|text| serde_json::from_str::<State>(&text).ok())
        .map(|state| state.autonomous_loop)
        .unwrap_or_default()
}

/// Decides a Stop event. Returns 2 (keep going) only while the state is `running`,
/// no report exists, the hook has not already continued this turn, no background
/// work is pending, and a driver is live. Returns 0 in every other case.
///
/// `stdin` carries the hook payload as JSON. Malformed input counts as no payload.
pub fn exit_code(root: &Path, mut stdin: impl Read, stderr: &mut impl Write) -> i32 {
    let mut raw = Vec::new();
    let input: HookInput = match stdin.read_to_end(&mut raw) {
        Ok(_) => serde_json::from_slice(&raw).unwrap_or_default(),
        Err(_) => HookInput::default(),
    };

    let run = read_loop_run(&root.join(".plaesy").join("state.json"));
    if run.status != "running" {
        return 0;
    }
    let loop_dir = root.join(LOOP_REPORT_DIR);
    if [COMPLETION_REPORT_FILE, STOP_REPORT_FILE]
        .iter()
        .any(|name| loop_dir.join(name).exists())
    {
        return 0;
    }
    if input.stop_hook_active {
        return 0;
    }
    if !input.background_tasks.is_empty() || !input.session_crons.is_empty() {
        return 0;
    }
    if !run.is_live(Utc::now()) {
        let _ = writeln!(
            stderr,
            "plaesy loop: the run is not live (no recent heartbeat from a driver), so the stop was not enforced."
        );
        return 0;
    }
    let _ = writeln!(
        stderr,
        "plaesy loop: the run is still running. Continue with the next iteration, or write the Stop or Completion report."
    );
    2
}

/// Reports whether a process with `pid` exists.
#[cfg(unix)]
fn pid_alive(pid: i64) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    // SAFETY: kill with signal 0 performs only an existence and permission check.
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Reports whether a process with `pid` exists, by opening a query handle to it.
#[cfg(windows)]
fn pid_alive(pid: i64) -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    let Ok(pid) = u32::try_from(pid) else {
        return false;
    };
    if pid == 0 {
        return false;
    }
    // SAFETY: OpenProcess with a query-only right has no side effects; a null handle means absent.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    // SAFETY: the handle was just returned by OpenProcess and is closed exactly once here.
    unsafe { CloseHandle(handle) };
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn write_state(root: &Path, run: &str) {
        fs::create_dir_all(root.join(".plaesy")).unwrap();
        fs::write(
            root.join(".plaesy/state.json"),
            format!(r#"{{"autonomous_loop":{run}}}"#),
        )
        .unwrap();
    }

    #[test]
    fn no_state_file_does_not_block() {
        let dir = tempfile::tempdir().unwrap();
        let mut err = Vec::new();
        assert_eq!(exit_code(dir.path(), &b""[..], &mut err), 0);
    }

    #[test]
    fn a_report_ends_the_turn() {
        let dir = tempfile::tempdir().unwrap();
        write_state(dir.path(), r#"{"status":"running"}"#);
        fs::create_dir_all(dir.path().join(LOOP_REPORT_DIR)).unwrap();
        fs::write(
            dir.path().join(LOOP_REPORT_DIR).join(STOP_REPORT_FILE),
            "done",
        )
        .unwrap();
        let mut err = Vec::new();
        assert_eq!(exit_code(dir.path(), &b""[..], &mut err), 0);
    }

    #[test]
    fn already_continued_turn_is_released() {
        let dir = tempfile::tempdir().unwrap();
        write_state(dir.path(), r#"{"status":"running"}"#);
        let mut err = Vec::new();
        assert_eq!(
            exit_code(dir.path(), &br#"{"stop_hook_active":true}"#[..], &mut err),
            0
        );
    }

    #[test]
    fn stale_heartbeat_is_not_live() {
        let run = LoopRun {
            status: "running".into(),
            driver_pid: std::process::id().into(),
            heartbeat_at: (Utc::now() - Duration::hours(1)).to_rfc3339(),
        };
        assert!(!run.is_live(Utc::now()));
    }

    #[test]
    fn a_live_driver_keeps_the_turn_going() {
        let dir = tempfile::tempdir().unwrap();
        let heartbeat = Utc::now().to_rfc3339();
        write_state(
            dir.path(),
            &format!(
                r#"{{"status":"running","driver_pid":{},"heartbeat_at":"{heartbeat}"}}"#,
                std::process::id()
            ),
        );
        let mut err = Vec::new();
        assert_eq!(exit_code(dir.path(), &b"{}"[..], &mut err), 2);
        assert!(String::from_utf8(err).unwrap().contains("still running"));
    }

    #[test]
    fn pending_background_work_releases_the_stop() {
        let dir = tempfile::tempdir().unwrap();
        let heartbeat = Utc::now().to_rfc3339();
        write_state(
            dir.path(),
            &format!(
                r#"{{"status":"running","driver_pid":{},"heartbeat_at":"{heartbeat}"}}"#,
                std::process::id()
            ),
        );
        let mut err = Vec::new();
        assert_eq!(
            exit_code(
                dir.path(),
                &br#"{"background_tasks":[{"id":1}]}"#[..],
                &mut err
            ),
            0
        );
    }
}
