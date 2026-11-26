//! Unit tests for the local `doctor` checks, included via `#[path]` from `doctor.rs`.
//! Each check has a failing case; assertions use the status, not the message text.

use super::*;
use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

// ---- Binary version ----

#[test]
fn version_check_passes_for_semver_core() {
    assert_eq!(check_version("0.0.1").status, Status::Pass);
    assert_eq!(check_version("12.3.45-rc.1").status, Status::Pass);
}

#[test]
fn version_check_fails_for_empty_version() {
    assert_eq!(check_version("").status, Status::Fail);
}

#[test]
fn version_check_fails_for_incomplete_version() {
    assert_eq!(check_version("1.2").status, Status::Fail);
}

#[test]
fn version_check_fails_for_non_numeric_part() {
    assert_eq!(check_version("1.x.3").status, Status::Fail);
}

// ---- Memory vault ----

#[test]
fn vault_check_passes_when_readable() {
    let check = check_vault(Path::new(".plaesy"), &VaultState::Readable);
    assert_eq!(check.status, Status::Pass);
}

#[test]
fn vault_check_fails_when_unconfigured() {
    let check = check_vault(Path::new(""), &VaultState::Unconfigured);
    assert_eq!(check.status, Status::Fail);
}

#[test]
fn vault_check_fails_when_config_cannot_load() {
    let state = VaultState::ConfigError("bad toml".into());
    assert_eq!(check_vault(Path::new(""), &state).status, Status::Fail);
}

#[test]
fn vault_check_fails_when_missing() {
    let check = check_vault(Path::new("/no/such/vault"), &VaultState::Missing);
    assert_eq!(check.status, Status::Fail);
}

#[test]
fn vault_check_fails_when_not_a_directory() {
    let check = check_vault(Path::new("notes.md"), &VaultState::NotDirectory);
    assert_eq!(check.status, Status::Fail);
}

#[test]
fn vault_check_fails_when_unreadable() {
    let state = VaultState::Unreadable("permission denied".into());
    assert_eq!(check_vault(Path::new("vault"), &state).status, Status::Fail);
}

#[test]
fn probe_vault_reports_missing_directory() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("absent");
    assert!(matches!(probe_vault(&missing), VaultState::Missing));
}

#[test]
fn probe_vault_reports_file_as_not_directory() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("note.md");
    std::fs::write(&file, "x").unwrap();
    assert!(matches!(probe_vault(&file), VaultState::NotDirectory));
}

#[test]
fn probe_vault_reports_empty_path_as_unconfigured() {
    assert!(matches!(
        probe_vault(Path::new("")),
        VaultState::Unconfigured
    ));
}

#[test]
fn probe_vault_reports_existing_directory_as_readable() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(probe_vault(dir.path()), VaultState::Readable));
}

// ---- Orphaned daemon lock ----

fn lock_with_age(secs: u64) -> LockState {
    LockState::Directory {
        age: Some(Duration::from_secs(secs)),
    }
}

#[test]
fn daemon_lock_absent_passes() {
    assert_eq!(check_daemon_lock(&LockState::Absent).status, Status::Pass);
}

#[test]
fn daemon_lock_fresh_directory_passes() {
    assert_eq!(check_daemon_lock(&lock_with_age(1)).status, Status::Pass);
}

#[test]
fn daemon_lock_at_limit_passes() {
    assert_eq!(check_daemon_lock(&lock_with_age(15)).status, Status::Pass);
}

#[test]
fn daemon_lock_older_than_limit_is_orphaned() {
    assert_eq!(check_daemon_lock(&lock_with_age(16)).status, Status::Fail);
}

#[test]
fn daemon_lock_with_unknown_age_fails() {
    let state = LockState::Directory { age: None };
    assert_eq!(check_daemon_lock(&state).status, Status::Fail);
}

#[test]
fn daemon_lock_that_is_a_file_fails() {
    assert_eq!(
        check_daemon_lock(&LockState::NotDirectory).status,
        Status::Fail
    );
}

#[test]
fn daemon_lock_that_cannot_be_inspected_fails() {
    let state = LockState::Unreadable("access denied".into());
    assert_eq!(check_daemon_lock(&state).status, Status::Fail);
}

#[test]
fn probe_daemon_lock_absent_when_no_lock_exists() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(probe_daemon_lock(dir.path()), LockState::Absent));
}

#[test]
fn probe_daemon_lock_detects_file_in_place_of_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(DAEMON_LOCK_NAME), "x").unwrap();
    assert!(matches!(
        probe_daemon_lock(dir.path()),
        LockState::NotDirectory
    ));
}

#[test]
fn probe_daemon_lock_reports_fresh_directory_as_live() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(DAEMON_LOCK_NAME)).unwrap();
    let check = check_daemon_lock(&probe_daemon_lock(dir.path()));
    assert_eq!(check.status, Status::Pass);
}

// ---- Free disk space ----

#[test]
fn disk_check_passes_at_minimum() {
    assert_eq!(
        check_disk_space(Ok(MIN_FREE_BYTES), MIN_FREE_BYTES).status,
        Status::Pass
    );
}

#[test]
fn disk_check_fails_below_minimum() {
    assert_eq!(
        check_disk_space(Ok(MIN_FREE_BYTES - 1), MIN_FREE_BYTES).status,
        Status::Fail
    );
}

#[test]
fn disk_check_fails_when_probe_errors() {
    let probe = Err("query failed".to_string());
    assert_eq!(check_disk_space(probe, MIN_FREE_BYTES).status, Status::Fail);
}

#[test]
fn available_bytes_reports_space_for_existing_directory() {
    let dir = tempfile::tempdir().unwrap();
    let free = crate::common::disk::available_bytes(dir.path()).unwrap();
    assert!(free > 0);
}

// ---- git and tmux on PATH ----

#[test]
fn git_check_fails_when_missing() {
    assert_eq!(check_git(&ToolState::Missing).status, Status::Fail);
}

#[test]
fn git_check_fails_when_broken() {
    let state = ToolState::Broken("exec format error".into());
    assert_eq!(check_git(&state).status, Status::Fail);
}

#[test]
fn git_check_passes_with_version() {
    let state = ToolState::Found("git version 2.45.1".into());
    let check = check_git(&state);
    assert_eq!(check.status, Status::Pass);
    assert_eq!(check.detail, "git version 2.45.1");
}

#[test]
fn tmux_check_skips_when_unsupported_even_if_found() {
    let state = ToolState::Found("tmux 3.4".into());
    assert_eq!(check_tmux(&state, false).status, Status::Skip);
    assert_eq!(check_tmux(&ToolState::Missing, false).status, Status::Skip);
}

#[test]
fn tmux_check_skips_when_missing_or_broken() {
    assert_eq!(check_tmux(&ToolState::Missing, true).status, Status::Skip);
    let broken = ToolState::Broken("exited with 1".into());
    assert_eq!(check_tmux(&broken, true).status, Status::Skip);
}

#[test]
fn tmux_check_passes_with_version() {
    let state = ToolState::Found("tmux 3.4".into());
    assert_eq!(check_tmux(&state, true).status, Status::Pass);
}

#[test]
fn find_on_path_returns_none_for_empty_path() {
    assert!(find_on_path("git", OsStr::new("")).is_none());
}

#[test]
fn find_on_path_returns_none_when_file_is_absent() {
    let dir = tempfile::tempdir().unwrap();
    assert!(find_on_path("git", dir.path().as_os_str()).is_none());
}

#[test]
fn find_on_path_finds_platform_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let file = if cfg!(windows) { "git.exe" } else { "git" };
    std::fs::write(dir.path().join(file), "x").unwrap();
    let found = find_on_path("git", dir.path().as_os_str());
    assert_eq!(found, Some(dir.path().join(file)));
}

#[test]
fn probe_tool_reports_missing_for_empty_path() {
    assert_eq!(
        probe_tool("git", "--version", OsStr::new("")),
        ToolState::Missing
    );
}

#[cfg(unix)]
#[test]
fn probe_tool_reads_version_from_fake_path() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("git");
    std::fs::write(&script, "#!/bin/sh\necho 'git version 9.9.9'\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let state = probe_tool("git", "--version", dir.path().as_os_str());
    assert_eq!(state, ToolState::Found("git version 9.9.9".into()));
}
