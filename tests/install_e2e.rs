//! Opt-in post-bootstrap lifecycle test using the runner's real user profile and PATH.
//!
//! The ignored test writes to the current user's persistent PATH. Run it only on a
//! disposable CI runner or when intentionally verifying the current account's install.
//! It requires `PLAESY_INSTALL_E2E=1` and `PLAESY_E2E_BINARY` to point to an extracted
//! release bundle.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct InstallScenario {
    home: PathBuf,
    install_dir: PathBuf,
    installed_path: PathBuf,
    data_root: PathBuf,
    source_binary: PathBuf,
    sentinel: PathBuf,
    created_data_root: bool,
    cleanup_needed: bool,
}

impl InstallScenario {
    fn new() -> Self {
        assert_eq!(
            env::var("PLAESY_INSTALL_E2E").as_deref(),
            Ok("1"),
            "set PLAESY_INSTALL_E2E=1 to opt in"
        );

        let source_binary = env::var_os("PLAESY_E2E_BINARY")
            .map(PathBuf::from)
            .expect("PLAESY_E2E_BINARY must point to the extracted release binary");
        assert!(
            source_binary.is_file(),
            "release binary missing: {}",
            source_binary.display()
        );
        assert!(
            source_binary.is_absolute(),
            "release binary path must be absolute"
        );

        let home = if cfg!(windows) {
            env::var_os("USERPROFILE")
        } else {
            env::var_os("HOME")
        }
        .map(PathBuf::from)
        .expect("the OS user profile must be available");
        let install_dir = if cfg!(windows) {
            env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .expect("LOCALAPPDATA must be set on Windows")
                .join("Plaesy")
                .join("bin")
        } else {
            home.join(".local").join("bin")
        };
        let installed_path = install_dir.join(if cfg!(windows) {
            "plaesy.exe"
        } else {
            "plaesy"
        });
        assert!(
            installed_path.is_file(),
            "installer did not create {}",
            installed_path.display()
        );
        assert_eq!(
            fs::read(&installed_path).expect("read installed binary"),
            fs::read(&source_binary).expect("read extracted release binary"),
            "installed executable differs from the release bundle"
        );

        let data_root = if cfg!(windows) {
            env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .expect("LOCALAPPDATA must be set on Windows")
                .join("plaesy")
        } else if cfg!(target_os = "macos") {
            home.join("Library")
                .join("Application Support")
                .join("plaesy")
        } else {
            env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"))
                .join("plaesy")
        };
        let created_data_root = !data_root.exists();
        let sentinel = data_root.join(format!("install-e2e-{}.keep", std::process::id()));
        assert!(!sentinel.exists(), "E2E sentinel already exists");

        Self {
            home,
            install_dir,
            installed_path,
            data_root,
            source_binary,
            sentinel,
            created_data_root,
            cleanup_needed: true,
        }
    }

    fn run_binary(&self, binary: &Path, args: &[&str], input: &[u8]) -> Output {
        let mut command = Command::new(binary);
        command
            .args(args)
            .env_remove("MEMORY_DB_PATH")
            .env_remove("MEMORY_WATCH_DIR")
            .env_remove("PLAESY_MEMORY_VAULT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().expect("start Plaesy command");
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(input)
            .expect("write command input");
        child.wait_with_output().expect("wait for Plaesy command")
    }

    fn run_source(&self, args: &[&str], input: &[u8]) -> Output {
        self.run_binary(&self.source_binary, args, input)
    }

    fn run_ok(&self, binary: &Path, args: &[&str], input: &[u8]) -> Output {
        let output = self.run_binary(binary, args, input);
        assert!(
            output.status.success(),
            "plaesy {args:?} failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn assert_persisted_path(&self, expected: bool) {
        if cfg!(windows) && !expected {
            let script = r#"$managed = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Plaesy\bin')).TrimEnd([char]92); $entries = [Environment]::GetEnvironmentVariable('Path','User') -split ';'; $matches = @($entries | Where-Object { $_.Trim().Trim('"').TrimEnd([char]92) -ieq $managed }); if ($matches.Count -gt 0) { exit 0 } else { exit 1 }"#;
            let output = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .output()
                .expect("inspect the persisted Windows user PATH");
            assert_eq!(
                output.status.success(),
                expected,
                "managed install directory is still present in Windows user PATH"
            );
            return;
        }

        if !cfg!(windows) && !expected {
            let startup_files = [
                self.home.join(".bashrc"),
                self.home.join(".bash_profile"),
                self.home.join(".bash_login"),
                self.home.join(".profile"),
                self.home.join(".zshrc"),
                self.home.join(".zprofile"),
                self.home.join(".config/fish/config.fish"),
                self.home.join(".config/nushell/config.nu"),
            ];
            let entry_remains = startup_files.iter().any(|profile| {
                fs::read_to_string(profile).is_ok_and(|text| {
                    text.lines()
                        .any(|line| line == "# Added by the plaesy installer")
                })
            });
            assert!(
                !entry_remains,
                "managed PATH block remains in a shell profile"
            );
            return;
        }

        let output = if cfg!(windows) {
            let script = if expected {
                "$userPath = [Environment]::GetEnvironmentVariable('Path','User'); $env:Path = $userPath + ';' + $env:Path; (Get-Command plaesy -ErrorAction Stop).Source"
            } else {
                unreachable!("the false Windows case checks the persisted PATH directly")
            };
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .output()
                .expect("start a new PowerShell process")
        } else {
            let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
            let shell_name = Path::new(&shell)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("sh");
            let (args, command) = match shell_name {
                "bash" => (vec!["-lc", "command -v plaesy"], "bash login shell"),
                "zsh" => (vec!["-ic", "command -v plaesy"], "zsh interactive shell"),
                "fish" => (vec!["-ic", "type -p plaesy"], "fish interactive shell"),
                "nu" | "nushell" => (vec!["-c", "which plaesy | get path.0"], "Nushell"),
                _ => (vec!["-lc", "command -v plaesy"], "login shell"),
            };
            Command::new(shell)
                .args(args)
                .output()
                .unwrap_or_else(|error| panic!("start {command}: {error}"))
        };

        assert_eq!(
            output.status.success(),
            expected,
            "unexpected command lookup result\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if expected {
            let resolved = String::from_utf8_lossy(&output.stdout);
            let resolved = resolved.lines().last().unwrap_or_default().trim();
            assert_eq!(
                normalized_path(resolved),
                normalized_path(&self.installed_path.display().to_string()),
                "new shell resolved an unexpected plaesy binary"
            );
        }
    }

    fn assert_single_persisted_path_entry(&self) {
        if cfg!(windows) {
            let output = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[Environment]::GetEnvironmentVariable('Path','User')",
                ])
                .output()
                .expect("read Windows user PATH");
            assert!(output.status.success(), "could not read Windows user PATH");
            let entries = String::from_utf8_lossy(&output.stdout);
            let target = normalized_path(&self.install_dir.display().to_string());
            let matches = entries
                .trim()
                .split(';')
                .filter(|entry| normalized_path(entry.trim().trim_matches('"')) == target)
                .count();
            assert_eq!(
                matches, 1,
                "expected one managed entry in Windows user PATH"
            );
        } else {
            let startup_files = [
                self.home.join(".bashrc"),
                self.home.join(".bash_profile"),
                self.home.join(".bash_login"),
                self.home.join(".profile"),
                self.home.join(".zshrc"),
                self.home.join(".zprofile"),
                self.home.join(".config/fish/config.fish"),
                self.home.join(".config/nushell/config.nu"),
            ];
            let mut managed_files = 0;
            for profile in startup_files {
                let Ok(text) = fs::read_to_string(&profile) else {
                    continue;
                };
                let blocks = text
                    .lines()
                    .filter(|line| *line == "# Added by the plaesy installer")
                    .count();
                assert!(
                    blocks <= 1,
                    "duplicate installer blocks in {}",
                    profile.display()
                );
                managed_files += blocks;
            }
            assert!(managed_files > 0, "installer did not persist a PATH entry");
        }
    }
}

impl Drop for InstallScenario {
    fn drop(&mut self) {
        if self.cleanup_needed {
            // Best effort in case an assertion interrupts the normal uninstall path.
            let _ = self.run_source(&["uninstall"], b"y\n");
        }
        let _ = fs::remove_file(&self.sentinel);
        if self.created_data_root {
            let _ = fs::remove_dir(&self.data_root);
        }
    }
}

#[test]
#[ignore = "writes the current user's persistent PATH; use a disposable account"]
fn release_bundle_installs_repairs_upgrades_and_uninstalls_with_the_real_os_path() {
    let mut scenario = InstallScenario::new();

    scenario.assert_persisted_path(true);
    scenario.run_ok(&scenario.source_binary, &["status"], b"");

    fs::create_dir_all(&scenario.data_root).expect("create user data directory");
    fs::write(&scenario.sentinel, "preserve Plaesy data during uninstall")
        .expect("write user data sentinel");
    scenario.run_ok(&scenario.installed_path, &["--version"], b"");
    scenario.run_ok(&scenario.installed_path, &["--help"], b"");
    scenario.run_ok(&scenario.installed_path, &["memory", "--help"], b"");

    let vault = tempfile::tempdir().expect("create isolated memory vault");
    let memory_data = tempfile::tempdir().expect("create isolated memory database directory");
    fs::create_dir_all(vault.path().join("notes")).expect("create note directory");
    let note_path = vault.path().join("notes/archive-e2e.md");
    fs::write(
        &note_path,
        "---\nid: install-e2e-note\nstatus: active\n---\n\nE2EArchiveNeedle.\n",
    )
    .expect("write lifecycle test note");
    let config_path = vault.path().join("memory.toml");
    fs::write(
        &config_path,
        "[search]\nenable_vector_search = false\nenable_keyword_search = true\n",
    )
    .expect("write keyword-only memory config");
    let vault_arg = vault.path().to_string_lossy().into_owned();
    let config_arg = config_path.to_string_lossy().into_owned();
    let db_arg = memory_data
        .path()
        .join("memory.db")
        .to_string_lossy()
        .into_owned();
    let fingerprint_args = [
        "memory",
        "--vault",
        &vault_arg,
        "--config",
        &config_arg,
        "--db",
        &db_arg,
        "fingerprint",
        "notes/archive-e2e.md",
    ];
    let fingerprint_output = scenario.run_ok(&scenario.installed_path, &fingerprint_args, b"");
    let fingerprint_text = String::from_utf8_lossy(&fingerprint_output.stdout);
    let fingerprint = fingerprint_text
        .split_whitespace()
        .next()
        .expect("fingerprint hash is printed")
        .to_owned();
    assert_eq!(fingerprint.len(), 64);
    let archive_args = [
        "memory",
        "--vault",
        &vault_arg,
        "--config",
        &config_arg,
        "--db",
        &db_arg,
        "archive",
        "notes/archive-e2e.md",
        "--if-match",
        &fingerprint,
        "--reason",
        "completed",
        "--yes",
    ];
    scenario.run_ok(&scenario.installed_path, &archive_args, b"");
    assert!(
        fs::read_to_string(&note_path)
            .expect("read archived note")
            .contains("status: archived"),
        "installed CLI did not archive the note"
    );

    scenario.run_ok(&scenario.source_binary, &["install"], b"");
    scenario.assert_persisted_path(true);
    scenario.assert_single_persisted_path_entry();
    scenario.run_ok(&scenario.installed_path, &["upgrade"], b"");

    fs::remove_file(&scenario.installed_path).expect("remove install to exercise repair");
    scenario.run_ok(&scenario.source_binary, &["repair"], b"");
    assert_eq!(
        fs::read(&scenario.installed_path).expect("repair restores installed binary"),
        fs::read(&scenario.source_binary).expect("read release binary")
    );

    let mut stale = OpenOptions::new()
        .append(true)
        .open(&scenario.installed_path)
        .expect("open installed binary to simulate an older build");
    stale
        .write_all(b"stale-install-marker")
        .expect("mark stale install");
    drop(stale);
    scenario.run_ok(&scenario.source_binary, &["upgrade"], b"");
    assert_eq!(
        fs::read(&scenario.installed_path).expect("upgrade restores release binary"),
        fs::read(&scenario.source_binary).expect("read release binary")
    );

    scenario.run_ok(&scenario.source_binary, &["uninstall"], b"y\n");
    scenario.cleanup_needed = false;
    assert!(
        !scenario.installed_path.exists(),
        "uninstall leaves the binary behind"
    );
    assert!(scenario.sentinel.is_file(), "uninstall removed user data");
    scenario.assert_persisted_path(false);
    scenario.run_ok(&scenario.source_binary, &["status"], b"");
}

fn normalized_path(path: &str) -> String {
    let path = Path::new(path).components().collect::<PathBuf>();
    let normalized = path
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_owned();
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}
