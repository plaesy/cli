//! The `plaesy install`, `status`, `uninstall`, `repair`, and `upgrade` commands.
//!
//! The running binary is the deliverable. `install` copies it to a per-OS bin
//! directory and extracts the embedded Plaesy home. `repair` and `upgrade` do the
//! same when the copies differ. Nothing needs the network.
//! Port of `scripts/internal/installer` (Go).

mod path_cleanup;

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::assets;
use crate::common::log::Logger;

pub use path_cleanup::remove_from_user_path;

/// Name of the installed executable, without extension.
pub const BINARY_NAME: &str = "plaesy";

/// Errors from the install family.
#[derive(Debug, Error)]
pub enum InstallerError {
    /// No home directory is available to derive install paths from.
    #[error("cannot resolve a home directory")]
    NoHomeDir,
    /// A filesystem operation failed.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    /// The running build is the installed copy, so there is nothing newer to upgrade from.
    #[error("this is the installed copy; run 'plaesy upgrade' from a newer build (e.g. the one in scripts/)")]
    RunningFromInstall,
    /// The Windows user PATH could not be read or written.
    #[error("{0}")]
    Registry(String),
}

impl InstallerError {
    pub(crate) fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

fn user_home() -> Result<PathBuf, InstallerError> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or(InstallerError::NoHomeDir)
}

fn local_app_data() -> Result<PathBuf, InstallerError> {
    match env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
        Some(base) => Ok(PathBuf::from(base)),
        None => Ok(user_home()?.join("AppData").join("Local")),
    }
}

/// Directory the binary is installed into. `%LOCALAPPDATA%\Plaesy\bin` on
/// Windows, `~/.local/bin` elsewhere.
///
/// # Errors
///
/// [`InstallerError::NoHomeDir`] when no home directory can be found.
pub fn install_dir() -> Result<PathBuf, InstallerError> {
    if cfg!(windows) {
        return Ok(local_app_data()?.join("Plaesy").join("bin"));
    }
    Ok(user_home()?.join(".local").join("bin"))
}

/// Directory Plaesy owns for its data. `%LOCALAPPDATA%\Plaesy` on Windows,
/// `~/.local/share/plaesy` elsewhere.
///
/// # Errors
///
/// As [`install_dir`].
pub fn app_data_dir() -> Result<PathBuf, InstallerError> {
    if cfg!(windows) {
        return Ok(local_app_data()?.join("Plaesy"));
    }
    Ok(user_home()?.join(".local").join("share").join("plaesy"))
}

/// The managed Plaesy home: [`app_data_dir`]`/.plaesy`. `install` fills it from the
/// embedded assets, and `init` falls back to it.
///
/// # Errors
///
/// As [`app_data_dir`].
pub fn plaesy_home_dir() -> Result<PathBuf, InstallerError> {
    Ok(app_data_dir()?.join(".plaesy"))
}

fn binary_file_name() -> String {
    if cfg!(windows) {
        format!("{BINARY_NAME}.exe")
    } else {
        BINARY_NAME.to_owned()
    }
}

/// Full path of the installed binary.
///
/// # Errors
///
/// As [`install_dir`].
pub fn installed_path() -> Result<PathBuf, InstallerError> {
    Ok(install_dir()?.join(binary_file_name()))
}

/// Reports whether `dir` is an entry of `PATH`. Windows compares case-insensitively.
#[must_use]
pub fn on_path(dir: &Path) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    let target = dir.components().collect::<PathBuf>();
    env::split_paths(&path).any(|entry| {
        let text = entry.to_string_lossy();
        let text = text.trim().trim_matches('"');
        if text.is_empty() {
            return false;
        }
        let candidate: PathBuf = Path::new(text).components().collect();
        if cfg!(windows) {
            candidate.to_string_lossy().to_lowercase() == target.to_string_lossy().to_lowercase()
        } else {
            candidate == target
        }
    })
}

/// Instructions for adding `dir` to `PATH`. Nothing is edited automatically.
#[must_use]
pub fn path_instructions(dir: &Path) -> String {
    let shown = dir.display();
    if cfg!(windows) {
        return format!(
            "\"{shown}\" is not on your PATH. Add it, then open a new terminal:\n  PowerShell: [Environment]::SetEnvironmentVariable('Path', $env:Path + ';{shown}', 'User')\n  nushell:    add to your env config (run `config env`): $env.PATH = ($env.PATH | prepend '{shown}' | uniq)"
        );
    }
    unix_path_instructions(dir, env::var("SHELL").unwrap_or_default().as_str())
}

/// Unix half of [`path_instructions`], with the shell passed in so it can be tested.
#[must_use]
pub fn unix_path_instructions(dir: &Path, shell: &str) -> String {
    let rc = if shell.contains("zsh") {
        "~/.zshrc"
    } else if shell.contains("bash") {
        "~/.bashrc (or ~/.bash_profile)"
    } else {
        "~/.profile"
    };
    format!(
        "\"{}\" is not on your PATH. Add this line to {rc}, then restart your shell:\n  export PATH=\"{}:$PATH\"",
        dir.display(),
        dir.display()
    )
}

/// Copies the running binary into the install directory and extracts the home.
/// Returns the installed binary path.
///
/// # Errors
///
/// Copy or extraction failures.
pub fn install(log: &Logger) -> Result<PathBuf, InstallerError> {
    let src = running_binary()?;
    let dst = install_from(&src)?;
    let home = plaesy_home_dir()?;
    assets::extract(&home, log).map_err(|e| InstallerError::Io {
        context: format!("extract bundled assets to {}", home.display()),
        source: io::Error::other(e.to_string()),
    })?;
    Ok(dst)
}

/// Copies `src` to the installed location. A copy onto itself is a no-op, so an
/// installed binary that runs `install` cannot truncate itself.
fn install_from(src: &Path) -> Result<PathBuf, InstallerError> {
    let dst = installed_path()?;
    if src == dst {
        return Ok(dst);
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| InstallerError::io("create install directory", e))?;
    }
    copy_file(src, &dst)?;
    Ok(dst)
}

/// Copies through a temporary file, then renames it over `dst`. The existing file is
/// removed first, because Windows refuses to rename over a file that is open.
fn copy_file(src: &Path, dst: &Path) -> Result<(), InstallerError> {
    let mut tmp = dst.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::copy(src, &tmp)
        .map_err(|e| InstallerError::io(format!("copy binary to {}", tmp.display()), e))?;
    let _ = fs::remove_file(dst);
    fs::rename(&tmp, dst).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        InstallerError::io(format!("copy binary to {}", dst.display()), e)
    })
}

/// Install state, for `plaesy status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub install_dir: PathBuf,
    pub install_path: PathBuf,
    pub installed: bool,
    pub on_path: bool,
    pub running_from: Option<PathBuf>,
    pub version: String,
}

/// Gathers the install state.
///
/// # Errors
///
/// [`InstallerError::NoHomeDir`] when the install directory cannot be resolved.
pub fn status(version: &str) -> Result<Status, InstallerError> {
    let install_dir = install_dir()?;
    let install_path = installed_path()?;
    Ok(Status {
        on_path: on_path(&install_dir),
        installed: install_path.is_file(),
        running_from: running_binary().ok(),
        install_dir,
        install_path,
        version: version.to_owned(),
    })
}

/// Reads one line from `input`, printing `prompt` first. Only `y` or `yes`
/// (case-insensitive) confirms; anything else, including EOF, declines.
pub fn confirm(input: impl BufRead, prompt: &str) -> bool {
    print!("{prompt}");
    let _ = io::stdout().flush();
    let mut lines = input.lines();
    match lines.next() {
        Some(Ok(line)) => matches!(line.trim().to_lowercase().as_str(), "y" | "yes"),
        _ => false,
    }
}

/// What [`uninstall`] removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub binary_path: PathBuf,
    pub path_dir: PathBuf,
    pub path_changed: bool,
}

/// Removes the installed binary, the managed home, and the PATH entry the installers
/// added. Project `.plaesy/` folders are not touched.
///
/// # Errors
///
/// Filesystem or PATH-update failures.
pub fn uninstall() -> Result<Removal, InstallerError> {
    let binary_path = installed_path()?;
    match fs::remove_file(&binary_path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(InstallerError::io(
                format!("remove {}", binary_path.display()),
                e,
            ))
        }
    }

    let app_dir = app_data_dir()?;
    if app_dir.exists() {
        fs::remove_dir_all(&app_dir)
            .map_err(|e| InstallerError::io(format!("remove {}", app_dir.display()), e))?;
    }

    let path_dir = install_dir()?;
    let path_changed = remove_from_user_path(&path_dir)?;
    Ok(Removal {
        binary_path,
        path_dir,
        path_changed,
    })
}

/// What [`repair`] or [`upgrade`] changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub binary_path: PathBuf,
    /// The installed binary was written because it was missing or different.
    pub binary_copied: bool,
    /// The managed home that was re-extracted.
    pub home_dir: PathBuf,
    pub on_path: bool,
}

/// Makes the installed binary and the managed home match the running build.
///
/// # Errors
///
/// Copy or extraction failures.
pub fn repair(log: &Logger) -> Result<Report, InstallerError> {
    sync_install(log)
}

/// Replaces the installed binary with the running build and refreshes the home.
///
/// # Errors
///
/// [`InstallerError::RunningFromInstall`] when run from the installed copy, plus
/// the errors of [`repair`].
pub fn upgrade(log: &Logger) -> Result<Report, InstallerError> {
    if running_binary()? == installed_path()? {
        return Err(InstallerError::RunningFromInstall);
    }
    sync_install(log)
}

fn sync_install(log: &Logger) -> Result<Report, InstallerError> {
    let src = running_binary()?;
    let dst = installed_path()?;
    let binary_copied = binary_differs(&src, &dst)?;
    if binary_copied {
        install_from(&src)?;
    }

    let home = plaesy_home_dir()?;
    assets::extract(&home, log).map_err(|e| InstallerError::Io {
        context: format!("extract bundled assets to {}", home.display()),
        source: io::Error::other(e.to_string()),
    })?;

    Ok(Report {
        binary_path: dst,
        binary_copied,
        home_dir: home,
        on_path: on_path(&install_dir()?),
    })
}

/// The running executable, with symlinks resolved so a copy and its original compare equal.
fn running_binary() -> Result<PathBuf, InstallerError> {
    let exe = env::current_exe().map_err(|e| InstallerError::io("locate running binary", e))?;
    exe.canonicalize()
        .map_err(|e| InstallerError::io("resolve running binary path", e))
}

/// Reports whether `dst` is missing or has different bytes from `src`.
fn binary_differs(src: &Path, dst: &Path) -> Result<bool, InstallerError> {
    if src == dst {
        return Ok(false);
    }
    if !dst.exists() {
        return Ok(true);
    }
    Ok(file_digest(src)? != file_digest(dst)?)
}

fn file_digest(path: &Path) -> Result<[u8; 32], InstallerError> {
    let bytes =
        fs::read(path).map_err(|e| InstallerError::io(format!("read {}", path.display()), e))?;
    Ok(Sha256::digest(bytes).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_instructions_name_the_shell_rc_file() {
        let dir = Path::new("/home/me/.local/bin");
        assert!(unix_path_instructions(dir, "/usr/bin/zsh").contains("~/.zshrc"));
        assert!(unix_path_instructions(dir, "/bin/bash").contains("~/.bashrc"));
        assert!(unix_path_instructions(dir, "").contains("~/.profile"));
    }

    #[test]
    fn copy_file_replaces_an_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");
        fs::write(&src, "new build").unwrap();
        fs::write(&dst, "old build").unwrap();
        copy_file(&src, &dst).unwrap();
        assert_eq!(fs::read_to_string(&dst).unwrap(), "new build");
        assert!(!dir.path().join("dst.tmp").exists());
    }

    #[test]
    fn differing_digest_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::write(&a, "one").unwrap();
        fs::write(&b, "one").unwrap();
        assert!(!binary_differs(&a, &b).unwrap());
        fs::write(&b, "two").unwrap();
        assert!(binary_differs(&a, &b).unwrap());
        assert!(binary_differs(&a, &dir.path().join("missing")).unwrap());
    }

    #[test]
    fn confirm_defaults_to_no() {
        assert!(confirm(io::Cursor::new("y\n"), ""));
        assert!(confirm(io::Cursor::new("YES\n"), ""));
        assert!(!confirm(io::Cursor::new("\n"), ""));
        assert!(!confirm(io::Cursor::new(""), ""));
    }
}
