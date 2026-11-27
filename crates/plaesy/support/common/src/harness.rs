//! Registry of the 40 agent harnesses listed in the Orchestrator reference, and a local readiness
//! check: is the harness executable on `PATH`, and which version does it report.
//!
//! Nothing here starts an agent session. `check` runs the located executable once with `--version`
//! under a deadline. A harness whose executable name the reference does not state is never run;
//! it is reported as unverified.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How long `--version` may run before the probe gives up and kills the process.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
/// Poll interval while waiting for the probe process.
const PROBE_POLL: Duration = Duration::from_millis(20);

/// One harness entry from the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Harness {
    /// Harness id as the Orchestrator reference names it, for example `claude-code`.
    pub id: &'static str,
    /// Executable name. `None` when the reference does not state it, or when the name is too
    /// generic to identify the harness (`cmd` is the Windows shell). Such an entry is not located.
    pub binary: Option<&'static str>,
    /// The harness has a terminal UI in the reference.
    pub tui: bool,
    /// The harness has a chat surface in the reference.
    pub chat: bool,
    /// The harness can act as the built-in reviewer in the reference.
    pub reviewer: bool,
    /// Minimum `MAJOR.MINOR.PATCH` version the reference requires.
    pub min_version: Option<&'static str>,
}

/// Builds a registry entry with no minimum version.
const fn entry(
    id: &'static str,
    binary: Option<&'static str>,
    tui: bool,
    chat: bool,
    reviewer: bool,
) -> Harness {
    Harness {
        id,
        binary,
        tui,
        chat,
        reviewer,
        min_version: None,
    }
}

impl Harness {
    /// Sets the minimum version the reference requires.
    const fn with_min(self, version: &'static str) -> Self {
        Self {
            min_version: Some(version),
            ..self
        }
    }
}

/// The 40 harnesses of AOR-082 to AOR-121, in reference order. Binary names are taken only from the
/// reference; the reference names an executable for 9 of them. The rest stay `None` until their
/// executable is verified.
pub const HARNESSES: &[Harness] = &[
    entry("claude-code", Some("claude"), true, true, true),
    entry("codex", Some("codex"), true, true, true),
    entry("opencode", Some("opencode"), true, true, true),
    entry("opencode-v2", Some("opencode"), true, true, true),
    entry("grok", Some("grok"), true, true, true),
    entry("cursor", Some("cursor-agent"), true, true, true),
    entry("qwen", None, true, true, false),
    entry("gemini", None, true, false, false),
    entry("copilot", None, true, false, true),
    entry("kimi", None, true, true, true),
    entry("muse", None, true, false, true),
    entry("droid", None, true, true, true),
    entry("amp", None, true, false, true),
    entry("agy", None, true, false, true),
    entry("crush", None, true, false, true),
    entry("aider", None, true, false, true),
    entry("goose", None, true, false, false),
    entry("auggie", None, true, false, true),
    entry("continue", None, true, false, false),
    entry("devin", None, true, false, true),
    entry("omp", None, true, true, false),
    entry("hermes", Some("hermes"), true, true, false),
    entry("cline", None, true, false, true),
    entry("kiro", None, true, false, true),
    entry("kilocode", None, true, false, true),
    entry("vibe", None, true, false, false),
    entry("pi", None, true, true, true),
    entry("kimchi", None, true, true, true),
    entry("prime-agent", None, true, false, false),
    entry("autohand", None, true, false, true),
    entry("fx", Some("fx"), true, false, false),
    entry("unreal-agent", None, false, true, false),
    entry("docker-agent", None, false, true, false).with_min("1.150.0"),
    entry("mimo-code", None, true, false, false).with_min("0.1.14"),
    entry("deepseek-harness", Some("dsh"), true, true, false),
    entry("openhands", None, true, false, false),
    entry("command-code", None, true, true, false),
    entry("mastra-code", None, true, false, false),
    entry("open-interpreter", None, true, false, false),
    entry("stakpak", None, true, false, false),
];

/// Result of checking one harness on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// The registry does not state an executable name, so nothing was looked up.
    Unverified,
    /// No executable with that name is on `PATH`.
    Missing,
    /// Found on `PATH`. `version` is `None` when the probe gave no parseable version.
    Found {
        path: PathBuf,
        version: Option<String>,
    },
    /// Found, but the reported version is below the minimum the reference requires.
    Outdated {
        path: PathBuf,
        version: String,
        min: &'static str,
    },
}

/// Returns the executable suffixes to try, in order, for the current platform. On Windows these
/// come from `PATHEXT`, so a `.cmd` shim is found and an extensionless script is not. Elsewhere the
/// name is used as it is.
#[must_use]
pub fn platform_exts(pathext: Option<&OsStr>) -> Vec<String> {
    if cfg!(windows) {
        let raw = pathext
            .map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_default();
        let exts: Vec<String> = raw
            .split(';')
            .map(str::trim)
            .filter(|e| e.starts_with('.'))
            .map(str::to_ascii_lowercase)
            .collect();
        if exts.is_empty() {
            return [".com", ".exe", ".bat", ".cmd"]
                .iter()
                .map(|e| (*e).to_owned())
                .collect();
        }
        exts
    } else {
        let _ = pathext;
        vec![String::new()]
    }
}

/// Finds the first regular, executable file named `binary` plus one of `exts` in the directories of
/// `path_var`, searched in order. Empty `PATH` entries are skipped.
#[must_use]
pub fn locate(binary: &str, path_var: &OsStr, exts: &[String]) -> Option<PathBuf> {
    for dir in env::split_paths(path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for ext in exts {
            let candidate = dir.join(format!("{binary}{ext}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    executable_bit(&meta)
}

#[cfg(unix)]
fn executable_bit(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable_bit(_meta: &fs::Metadata) -> bool {
    true
}

/// Pulls the first `MAJOR.MINOR[.PATCH...]` token out of a `--version` line, for example
/// `claude 1.2.3 (Claude Code)` gives `1.2.3`. A leading `v` is dropped.
#[must_use]
pub fn parse_version(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|token| {
        let start = token.find(|c: char| c.is_ascii_digit())?;
        let rest = &token[start..];
        let end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let core = rest[..end].trim_end_matches('.');
        let parts: Vec<&str> = core.split('.').collect();
        let well_formed = parts.len() >= 2
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        well_formed.then(|| core.to_owned())
    })
}

/// Compares two dotted numeric versions, padding the shorter one with zeros. Returns `None` when
/// either side has a non-numeric part.
#[must_use]
pub fn at_least(version: &str, min: &str) -> Option<bool> {
    let a = numeric_parts(version)?;
    let b = numeric_parts(min)?;
    let len = a.len().max(b.len());
    let pad = |v: &[u64]| -> Vec<u64> {
        let mut out = v.to_vec();
        out.resize(len, 0);
        out
    };
    Some(pad(&a) >= pad(&b))
}

fn numeric_parts(version: &str) -> Option<Vec<u64>> {
    version
        .split('.')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()
}

/// Runs `path --version` once. Returns the parsed version, or `None` when the process fails,
/// prints nothing usable, or runs past the deadline (it is killed then).
#[must_use]
pub fn probe_version(path: &Path) -> Option<String> {
    let mut child = Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = thread::spawn(move || {
        let mut buf = String::new();
        // A read error leaves whatever was read so far; parse_version handles an empty result.
        let _ = stdout.read_to_string(&mut buf);
        buf
    });
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let text = reader.join().ok()?;
                return parse_version(&text);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(PROBE_POLL);
            }
            Err(_) => return None,
        }
    }
}

/// Checks one harness against an explicit `PATH` and suffix list, using `probe` for the version.
/// The probe runs only when the executable was found.
pub fn check_with(
    harness: &Harness,
    path_var: &OsStr,
    exts: &[String],
    probe: impl FnOnce(&Path) -> Option<String>,
) -> Readiness {
    let Some(binary) = harness.binary else {
        return Readiness::Unverified;
    };
    let Some(path) = locate(binary, path_var, exts) else {
        return Readiness::Missing;
    };
    let version = probe(&path);
    if let (Some(min), Some(found)) = (harness.min_version, version.as_deref()) {
        if at_least(found, min) == Some(false) {
            return Readiness::Outdated {
                path,
                version: found.to_owned(),
                min,
            };
        }
    }
    Readiness::Found { path, version }
}

/// Checks one harness against the current process `PATH`.
#[must_use]
pub fn check(harness: &Harness) -> Readiness {
    let path_var = env::var_os("PATH").unwrap_or_default();
    let exts = platform_exts(env::var_os("PATHEXT").as_deref());
    check_with(harness, &path_var, &exts, probe_version)
}

#[cfg(test)]
#[path = "harness_test.rs"]
mod harness_test;
