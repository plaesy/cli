//! Running external commands under a deadline, and the shared shape of their results.

use std::io::{self, Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::types::{BackendHealth, BackendResult, Platform};

/// A point in time after which a call gives up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline(Instant);

impl Deadline {
    /// A deadline `after` from now.
    #[must_use]
    pub fn after(after: Duration) -> Self {
        Self(Instant::now() + after)
    }

    /// Time left, or `None` once the deadline has passed.
    #[must_use]
    pub fn remaining(self) -> Option<Duration> {
        self.0
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
    }
}

/// What a finished command produced.
pub struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs `args` (program first) with `env` added to the inherited environment.
///
/// The command is killed when `deadline` passes. `stdin` is written and then closed, when given.
///
/// # Errors
///
/// The program cannot be started, or the deadline passes.
pub fn capture(
    args: &[String],
    env: &[(String, String)],
    stdin: Option<&[u8]>,
    deadline: Deadline,
) -> io::Result<Captured> {
    let (program, rest) = args
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty command"))?;
    let mut cmd = Command::new(program);
    cmd.args(rest)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;

    if let (Some(data), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let data = data.to_vec();
        thread::spawn(move || {
            let _ = pipe.write_all(&data);
        });
    }
    let out_pipe = child.stdout.take();
    let err_pipe = child.stderr.take();
    let out_reader = thread::spawn(move || read_all(out_pipe));
    let err_reader = thread::spawn(move || read_all(err_pipe));

    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if deadline.remaining().is_none() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
        }
        thread::sleep(Duration::from_millis(20));
    };
    Ok(Captured {
        status,
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    })
}

fn read_all<R: Read>(pipe: Option<R>) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut buf);
    }
    buf
}

/// Runs a command and builds the backend result from it. Output is stdout then stderr.
#[must_use]
pub fn run_backend(
    platform: Platform,
    backend: &str,
    args: &[String],
    env: &[(String, String)],
    deadline: Deadline,
) -> BackendResult {
    let start = Instant::now();
    let outcome = capture(args, env, None, deadline);
    let mut result = BackendResult::new(backend, platform);
    result.latency = start.elapsed();
    match outcome {
        Ok(out) => {
            let mut bytes = out.stdout;
            bytes.extend_from_slice(&out.stderr);
            let text = String::from_utf8_lossy(&bytes).into_owned();
            if out.status.success() {
                result.success = true;
                result.data = text.trim().to_owned();
            } else {
                result.error = format!("{}: {text}", exit_text(out.status));
            }
        }
        Err(e) => result.error = format!("{e}: "),
    }
    result
}

/// Checks that `args` runs (exit status zero) within ten seconds, and reads `--version`.
#[must_use]
pub fn check_command(platform: Platform, backend: &str, args: &[&str]) -> BackendHealth {
    let start = Instant::now();
    let argv: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let outcome = capture(&argv, &[], None, Deadline::after(Duration::from_secs(10)));
    let mut health = BackendHealth::new(backend, platform);
    health.latency = start.elapsed();
    match outcome {
        Ok(out) if out.status.success() => {
            health.available = true;
            health.version = version_of(argv[0].as_str()).unwrap_or_default();
        }
        Ok(out) => health.error = exit_text(out.status),
        Err(e) => health.error = e.to_string(),
    }
    health
}

/// The trimmed output of `program --version`, or `None` when it fails.
#[must_use]
pub fn version_of(program: &str) -> Option<String> {
    let argv = vec![program.to_owned(), "--version".to_owned()];
    let out = capture(&argv, &[], None, Deadline::after(Duration::from_secs(10))).ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn exit_text(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("exit status {code}"),
        None => "terminated by signal".to_owned(),
    }
}

/// Splits a literal command line into owned arguments.
#[must_use]
pub fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_a_failed_result_not_a_panic() {
        let result = run_backend(
            Platform::Web,
            "missing",
            &argv(&["plaesy-no-such-program-xyz"]),
            &[],
            Deadline::after(Duration::from_secs(5)),
        );
        assert!(!result.success);
        assert!(!result.error.is_empty());
    }

    #[test]
    fn a_deadline_in_the_past_has_no_time_left() {
        let past = Deadline(Instant::now() - Duration::from_secs(1));
        assert!(past.remaining().is_none());
    }
}
