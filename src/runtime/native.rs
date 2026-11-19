//! Bounded native process adapter (spec R4, R9; best-practice default for the native runtime).
//!
//! Runs one allowlisted program per task with:
//! - a fixed working directory and a cleared environment (only a short allowlist is copied),
//! - no stdin, and stdout and stderr capped at `max_output_bytes`,
//! - a wall-clock timeout and cooperative cancellation, both of which kill the process,
//! - external effects off unless the deployment enables them.
//!
//! This is a host process with host permissions. Container or VM isolation is still
//! required before real tools run. The adapter is the bounded boundary, not a sandbox.

use crate::adapter::{CancelFlag, ExecutionAdapter, Outcome};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Environment variables copied from the parent process. Everything else is cleared.
const ENV_ALLOWLIST: &[&str] = &["PATH", "SYSTEMROOT", "TEMP", "TMP"];

/// Runs one allowlisted program as a bounded child process.
#[derive(Debug, Clone)]
pub struct NativeProcessAdapter {
    program: PathBuf,
    args: Vec<String>,
    work_dir: PathBuf,
    timeout: Duration,
    max_output_bytes: usize,
    allowlist: Vec<PathBuf>,
    external_effects_enabled: bool,
}

impl NativeProcessAdapter {
    /// Creates an adapter for `program`. External effects are off until enabled.
    #[must_use]
    pub fn new(
        program: impl Into<PathBuf>,
        args: Vec<String>,
        work_dir: impl Into<PathBuf>,
        timeout: Duration,
        allowlist: Vec<PathBuf>,
    ) -> Self {
        Self {
            program: program.into(),
            args,
            work_dir: work_dir.into(),
            timeout,
            max_output_bytes: 64 * 1024,
            allowlist,
            external_effects_enabled: false,
        }
    }

    /// Turns external effects on or off for this deployment.
    #[must_use]
    pub fn with_external_effects(mut self, enabled: bool) -> Self {
        self.external_effects_enabled = enabled;
        self
    }

    /// Sets the per-stream output cap in bytes.
    #[must_use]
    pub fn with_max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = max;
        self
    }
}

fn read_capped(reader: Option<impl Read>, cap: usize) -> Vec<u8> {
    let mut kept = Vec::new();
    let Some(mut reader) = reader else {
        return kept;
    };
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let room = cap.saturating_sub(kept.len());
                kept.extend_from_slice(&chunk[..read.min(room)]);
            }
        }
    }
    kept
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_allowed(allowlist: &[PathBuf], program: &Path) -> bool {
    allowlist.iter().any(|allowed| allowed == program)
}

impl ExecutionAdapter for NativeProcessAdapter {
    fn name(&self) -> &str {
        "native-process"
    }

    fn execute(&mut self, _task_id: &str, cancel: &CancelFlag) -> Outcome {
        if cancel.is_cancelled() {
            return Outcome::Cancelled;
        }
        if !self.external_effects_enabled {
            return Outcome::Failed {
                reason: "external_effects_disabled".to_owned(),
            };
        }
        if !is_allowed(&self.allowlist, &self.program) {
            return Outcome::Failed {
                reason: "program_not_allowed".to_owned(),
            };
        }

        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.work_dir)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in ENV_ALLOWLIST {
            if let Ok(value) = std::env::var(name) {
                command.env(name, value);
            }
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                return Outcome::Failed {
                    reason: format!("spawn_failed: {error}"),
                };
            }
        };

        let cap = self.max_output_bytes;
        let stdout_reader = child.stdout.take();
        let stderr_reader = child.stderr.take();
        let stdout_thread = thread::spawn(move || read_capped(stdout_reader, cap));
        let stderr_thread = thread::spawn(move || read_capped(stderr_reader, cap));

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(_) => break None,
            }
            let cancelled = cancel.is_cancelled();
            if cancelled || Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return if cancelled {
                    Outcome::Cancelled
                } else {
                    Outcome::TimedOut
                };
            }
            thread::sleep(Duration::from_millis(5));
        };

        let stdout = stdout_thread.join().unwrap_or_default();
        let _ = stderr_thread.join();
        match status {
            Some(status) if status.success() => Outcome::Succeeded {
                evidence: format!("exit=0 stdout_sha256={}", hex(&Sha256::digest(&stdout))),
            },
            Some(status) => Outcome::Failed {
                reason: format!("exit_code={}", status.code().unwrap_or(-1)),
            },
            None => Outcome::Failed {
                reason: "wait_failed".to_owned(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cargo() -> PathBuf {
        PathBuf::from(env!("CARGO"))
    }

    #[test]
    fn bounded_runtime_native_allowed_program_reports_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let mut adapter = NativeProcessAdapter::new(
            cargo(),
            vec!["--version".into()],
            dir.path(),
            Duration::from_secs(60),
            vec![cargo()],
        )
        .with_external_effects(true);
        match adapter.execute("task-1", &CancelFlag::new()) {
            Outcome::Succeeded { evidence } => {
                assert!(evidence.starts_with("exit=0 stdout_sha256="))
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[test]
    fn bounded_runtime_native_program_outside_allowlist_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut adapter = NativeProcessAdapter::new(
            cargo(),
            vec!["--version".into()],
            dir.path(),
            Duration::from_secs(60),
            Vec::new(),
        )
        .with_external_effects(true);
        assert_eq!(
            adapter.execute("task-1", &CancelFlag::new()),
            Outcome::Failed {
                reason: "program_not_allowed".into()
            }
        );
    }

    #[test]
    fn bounded_runtime_native_disabled_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let mut adapter = NativeProcessAdapter::new(
            cargo(),
            vec!["--version".into()],
            dir.path(),
            Duration::from_secs(60),
            vec![cargo()],
        );
        assert_eq!(
            adapter.execute("task-1", &CancelFlag::new()),
            Outcome::Failed {
                reason: "external_effects_disabled".into()
            }
        );
    }

    #[test]
    fn bounded_runtime_native_timeout_kills_process() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut adapter = NativeProcessAdapter::new(
            cargo(),
            vec!["metadata".into(), "--format-version".into(), "1".into()],
            manifest_dir,
            Duration::from_millis(1),
            vec![cargo()],
        )
        .with_external_effects(true);
        let _ = dir;
        assert_eq!(
            adapter.execute("task-1", &CancelFlag::new()),
            Outcome::TimedOut
        );
    }

    #[test]
    fn bounded_runtime_native_cancel_before_start_spawns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut adapter = NativeProcessAdapter::new(
            cargo(),
            vec!["--version".into()],
            dir.path(),
            Duration::from_secs(60),
            vec![cargo()],
        )
        .with_external_effects(true);
        let cancel = CancelFlag::new();
        cancel.cancel();
        assert_eq!(adapter.execute("task-1", &cancel), Outcome::Cancelled);
    }
}
