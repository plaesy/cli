//! Unit tests for `harness.rs`, included via `#[path]`. No test starts a real agent: the fake
//! executables are empty files, and the version probe is a closure.

use std::cell::Cell;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

const EXPECTED_IDS: [&str; 40] = [
    "claude-code",
    "codex",
    "opencode",
    "opencode-v2",
    "grok",
    "cursor",
    "qwen",
    "gemini",
    "copilot",
    "kimi",
    "muse",
    "droid",
    "amp",
    "agy",
    "crush",
    "aider",
    "goose",
    "auggie",
    "continue",
    "devin",
    "omp",
    "hermes",
    "cline",
    "kiro",
    "kilocode",
    "vibe",
    "pi",
    "kimchi",
    "prime-agent",
    "autohand",
    "fx",
    "unreal-agent",
    "docker-agent",
    "mimo-code",
    "deepseek-harness",
    "openhands",
    "command-code",
    "mastra-code",
    "open-interpreter",
    "stakpak",
];

/// Creates a file that `locate` accepts as executable on every platform.
fn make_exec(path: &Path) {
    fs::write(path, b"").expect("test fixture file can be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .expect("test fixture permissions can be set");
    }
}

fn fake_harness(binary: Option<&'static str>, min: Option<&'static str>) -> Harness {
    Harness {
        id: "fake",
        binary,
        tui: true,
        chat: false,
        reviewer: false,
        min_version: min,
    }
}

#[test]
fn registry_lists_the_forty_harnesses_in_reference_order() {
    let ids: Vec<&str> = HARNESSES.iter().map(|h| h.id).collect();
    assert_eq!(ids, EXPECTED_IDS);
}

#[test]
fn registry_ids_are_unique() {
    let mut ids: Vec<&str> = HARNESSES.iter().map(|h| h.id).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before);
}

#[test]
fn registry_minimum_versions_match_the_reference() {
    let min_of = |id: &str| {
        HARNESSES
            .iter()
            .find(|h| h.id == id)
            .and_then(|h| h.min_version)
    };
    assert_eq!(min_of("docker-agent"), Some("1.150.0"));
    assert_eq!(min_of("mimo-code"), Some("0.1.14"));
    assert_eq!(min_of("claude-code"), None);
}

#[test]
fn parse_version_takes_the_first_dotted_number() {
    assert_eq!(
        parse_version("claude 1.2.3 (Claude Code)\n"),
        Some("1.2.3".to_owned())
    );
    assert_eq!(parse_version("v0.1.14\n"), Some("0.1.14".to_owned()));
    assert_eq!(parse_version("tool 2.0.0-beta.1"), Some("2.0.0".to_owned()));
    assert_eq!(parse_version("no version here"), None);
    assert_eq!(parse_version(""), None);
}

#[test]
fn at_least_compares_numerically_with_zero_padding() {
    assert_eq!(at_least("1.150.0", "1.150.0"), Some(true));
    assert_eq!(at_least("1.99.0", "1.150.0"), Some(false));
    assert_eq!(at_least("0.1.13", "0.1.14"), Some(false));
    assert_eq!(at_least("0.1.14", "0.1.14"), Some(true));
    assert_eq!(at_least("2", "1.150.0"), Some(true));
    assert_eq!(at_least("1.2", "1.2.0"), Some(true));
    assert_eq!(at_least("x.y", "1.0.0"), None);
}

#[test]
fn locate_searches_path_directories_in_order() {
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");
    make_exec(&second.path().join("fake-agent.cmd"));
    let path_var = env::join_paths([first.path(), second.path()]).expect("join paths");
    let exts = vec![".cmd".to_owned()];

    let found = locate("fake-agent", &path_var, &exts);
    assert_eq!(found, Some(second.path().join("fake-agent.cmd")));
    assert_eq!(locate("fake-agent", &path_var, &[".exe".to_owned()]), None);
    assert_eq!(locate("other-agent", &path_var, &exts), None);
}

#[test]
fn locate_skips_directories_and_empty_path_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir(dir.path().join("dir-agent")).expect("create dir");
    let exe = dir.path().join("fake-agent");
    make_exec(&exe);
    let exts = [String::new()];

    let dir_only = env::join_paths([dir.path()]).expect("join paths");
    assert_eq!(locate("dir-agent", &dir_only, &exts), None);

    // An empty PATH entry means the current directory to some shells; it must be skipped, not used.
    let with_empty =
        env::join_paths([PathBuf::new(), dir.path().to_path_buf()]).expect("join paths");
    assert_eq!(locate("fake-agent", &with_empty, &exts), Some(exe));
}

#[test]
fn platform_exts_uses_pathext_on_windows_and_bare_names_elsewhere() {
    if cfg!(windows) {
        let exts = platform_exts(Some(OsStr::new(".COM;.EXE;;.CMD")));
        assert_eq!(exts, vec![".com", ".exe", ".cmd"]);
        assert_eq!(platform_exts(None), vec![".com", ".exe", ".bat", ".cmd"]);
    } else {
        assert_eq!(platform_exts(Some(OsStr::new(".EXE"))), vec![String::new()]);
    }
}

#[test]
fn check_reports_unverified_without_a_binary_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path_var = env::join_paths([dir.path()]).expect("join paths");
    let probed = Cell::new(false);
    let result = check_with(
        &fake_harness(None, None),
        &path_var,
        &[String::new()],
        |_| {
            probed.set(true);
            Some("1.0.0".to_owned())
        },
    );
    assert_eq!(result, Readiness::Unverified);
    assert!(!probed.get(), "an unverified harness is never probed");
}

#[test]
fn check_reports_missing_and_does_not_probe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path_var = env::join_paths([dir.path()]).expect("join paths");
    let probed = Cell::new(false);
    let result = check_with(
        &fake_harness(Some("fake-agent"), None),
        &path_var,
        &[String::new()],
        |_| {
            probed.set(true);
            None
        },
    );
    assert_eq!(result, Readiness::Missing);
    assert!(!probed.get());
}

#[test]
fn check_reports_found_with_the_probed_version() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exe = dir.path().join("fake-agent");
    make_exec(&exe);
    let path_var = env::join_paths([dir.path()]).expect("join paths");
    let result = check_with(
        &fake_harness(Some("fake-agent"), None),
        &path_var,
        &[String::new()],
        |p| {
            assert_eq!(p, exe.as_path());
            Some("3.4.5".to_owned())
        },
    );
    assert_eq!(
        result,
        Readiness::Found {
            path: exe,
            version: Some("3.4.5".to_owned()),
        }
    );
}

#[test]
fn check_reports_found_without_version_when_probe_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    make_exec(&dir.path().join("fake-agent"));
    let path_var = env::join_paths([dir.path()]).expect("join paths");
    let result = check_with(
        &fake_harness(Some("fake-agent"), Some("0.1.14")),
        &path_var,
        &[String::new()],
        |_| None,
    );
    assert!(matches!(result, Readiness::Found { version: None, .. }));
}

#[test]
fn check_reports_outdated_when_below_minimum() {
    let dir = tempfile::tempdir().expect("tempdir");
    let exe = dir.path().join("fake-agent");
    make_exec(&exe);
    let path_var = env::join_paths([dir.path()]).expect("join paths");
    let result = check_with(
        &fake_harness(Some("fake-agent"), Some("0.1.14")),
        &path_var,
        &[String::new()],
        |_| Some("0.1.13".to_owned()),
    );
    assert_eq!(
        result,
        Readiness::Outdated {
            path: exe,
            version: "0.1.13".to_owned(),
            min: "0.1.14",
        }
    );
}

#[test]
fn check_accepts_a_version_at_the_minimum() {
    let dir = tempfile::tempdir().expect("tempdir");
    make_exec(&dir.path().join("fake-agent"));
    let path_var = env::join_paths([dir.path()]).expect("join paths");
    let result = check_with(
        &fake_harness(Some("fake-agent"), Some("0.1.14")),
        &path_var,
        &[String::new()],
        |_| Some("0.1.14".to_owned()),
    );
    assert!(matches!(
        result,
        Readiness::Found { version: Some(ref v), .. } if v == "0.1.14"
    ));
}

#[test]
fn probe_version_returns_none_for_a_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing: PathBuf = dir.path().join("no-such-agent");
    assert_eq!(probe_version(&missing), None);
}
