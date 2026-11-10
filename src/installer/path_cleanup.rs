//! Adds and removes the PATH entries managed by the installer.
//!
//! Unix: the PATH block in the shell profiles, identified by its marker
//! line. Windows: the user `Path` value in `HKCU\Environment`. The machine PATH is
//! never touched.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::InstallerError;

/// Comment line the installer writes above its PATH entry.
pub const PROFILE_MARKER: &str = "# Added by the plaesy installer";

/// Adds `dir` to the current user's persistent PATH. Returns `true` if changed.
///
/// # Errors
///
/// [`InstallerError::Io`] for shell startup files and [`InstallerError::Registry`]
/// on Windows.
pub fn add_to_user_path(dir: &Path) -> Result<bool, InstallerError> {
    if cfg!(windows) {
        return add_to_registry(dir);
    }

    let shell = std::env::var("SHELL").unwrap_or_default();
    let line = if shell.contains("fish") {
        fish_path_line(dir)
    } else if is_nushell(shell.as_str()) {
        nushell_path_line(dir)
    } else {
        shell_path_line(dir)
    };

    let mut changed = false;
    for profile in install_profile_files(&shell)? {
        changed |= add_installer_lines(&profile, &line)?;
    }
    Ok(changed)
}

/// Removes `dir` from the user PATH. Returns `true` when the value changed.
///
/// # Errors
///
/// [`InstallerError::Io`] for profile files and [`InstallerError::Registry`] on Windows.
pub fn remove_from_user_path(dir: &Path) -> Result<bool, InstallerError> {
    if cfg!(windows) {
        return remove_from_registry(dir);
    }
    let mut removed = false;
    for profile in profile_files()? {
        removed |= remove_installer_lines(&profile, dir)?;
    }
    Ok(removed)
}

/// Drops every entry that names `dir`. Returns the new value when anything was removed.
#[must_use]
#[cfg(any(windows, test))]
pub fn drop_path_entry(value: &str, dir: &Path, fold: bool) -> Option<String> {
    let target = dir.components().collect::<PathBuf>();
    let mut removed = false;
    let kept: Vec<&str> = value
        .split(';')
        .filter(|entry| {
            let same = same_path_entry(entry, &target, fold);
            removed |= same;
            !same
        })
        .collect();
    removed.then(|| kept.join(";"))
}

#[cfg(any(windows, test))]
fn same_path_entry(entry: &str, target: &Path, fold: bool) -> bool {
    let candidate = entry.trim().trim_matches('"');
    if candidate.is_empty() {
        return false;
    }
    let expanded = if fold {
        expand_windows_environment(candidate)
    } else {
        candidate.to_owned()
    };
    let candidate: PathBuf = Path::new(&expanded).components().collect();
    if fold {
        candidate.to_string_lossy().to_lowercase() == target.to_string_lossy().to_lowercase()
    } else {
        candidate == target
    }
}

#[cfg(any(windows, test))]
fn expand_windows_environment(value: &str) -> String {
    let mut expanded = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let variable = &rest[start + 1..];
        let Some(end) = variable.find('%') else {
            expanded.push_str(&rest[start..]);
            return expanded;
        };
        let name = &variable[..end];
        if let Some(value) = std::env::var_os(name) {
            expanded.push_str(&value.to_string_lossy());
        } else {
            expanded.push_str(&rest[start..start + end + 2]);
        }
        rest = &variable[end + 1..];
    }
    expanded.push_str(rest);
    expanded
}

/// Removes the marker line and the export line after it from `profile`, plus the blank
/// line written before the marker. A missing profile is not an error.
///
/// # Errors
///
/// I/O failures reading or writing the profile.
pub fn remove_installer_lines(profile: &Path, dir: &Path) -> Result<bool, InstallerError> {
    let text = match fs::read_to_string(profile) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(InstallerError::io(format!("read {}", profile.display()), e)),
    };
    let path = dir.display().to_string();
    let want = [
        format!("export PATH=\"{path}:$PATH\""), // legacy installer format
        format!("case \":$PATH:\" in *:\"{path}\":*) ;; *) export PATH=\"{path}\":$PATH ;; esac"),
        shell_path_line(dir),
        format!("fish_add_path --path --prepend '{path}'"),
        fish_path_line(dir),
        nushell_path_line(dir),
    ];
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut changed = false;
    let mut i = 0;
    while i < lines.len() {
        if lines[i] == PROFILE_MARKER
            && lines
                .get(i + 1)
                .is_some_and(|line| want.iter().any(|entry| line == entry))
        {
            if out.last() == Some(&"") {
                out.pop();
            }
            changed = true;
            i += 2;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    if !changed {
        return Ok(false);
    }
    let mode = fs::metadata(profile)
        .map(|m| m.permissions())
        .map_err(|e| InstallerError::io(format!("stat {}", profile.display()), e))?;
    fs::write(profile, out.join("\n"))
        .map_err(|e| InstallerError::io(format!("write {}", profile.display()), e))?;
    fs::set_permissions(profile, mode)
        .map_err(|e| InstallerError::io(format!("set permissions {}", profile.display()), e))?;
    Ok(true)
}

/// Shell profiles the installer may have written to.
///
/// # Errors
///
/// [`InstallerError::NoHomeDir`] when no home directory is set.
pub fn profile_files() -> Result<Vec<PathBuf>, InstallerError> {
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or(InstallerError::NoHomeDir)?;
    let mut files = vec![
        home.join(".bashrc"),
        home.join(".bash_profile"),
        home.join(".bash_login"),
        home.join(".profile"),
        home.join(".zshrc"),
        home.join(".zprofile"),
        fish_config_file(&home),
    ];
    if let Ok(path) = nushell_config_file() {
        files.push(path);
    }
    Ok(files)
}

fn install_profile_files(shell: &str) -> Result<Vec<PathBuf>, InstallerError> {
    if is_nushell(shell) {
        return Ok(vec![nushell_config_file()?]);
    }
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or(InstallerError::NoHomeDir)?;
    if shell.contains("fish") {
        return Ok(vec![fish_config_file(&home)]);
    }
    if shell.contains("zsh") {
        return Ok(vec![home.join(".zshrc")]);
    }
    if shell.contains("bash") {
        let login_profile = [".bash_profile", ".bash_login", ".profile"]
            .into_iter()
            .map(|name| home.join(name))
            .find(|path| path.exists())
            .unwrap_or_else(|| home.join(".profile"));
        return Ok(vec![home.join(".bashrc"), login_profile]);
    }
    Ok(vec![home.join(".profile")])
}

fn fish_config_file(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("fish")
        .join("config.fish")
}

fn nushell_config_file() -> Result<PathBuf, InstallerError> {
    let config_dir = dirs::config_dir()
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .ok_or(InstallerError::NoHomeDir)?;
    Ok(config_dir.join("nushell").join("config.nu"))
}

pub(crate) fn is_nushell(shell: &str) -> bool {
    Path::new(shell)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("nu") || name.eq_ignore_ascii_case("nushell"))
}

fn shell_path_line(dir: &Path) -> String {
    let quoted = shell_single_quote(&dir.display().to_string());
    format!("case \":$PATH:\" in *:{quoted}:*) ;; *) export PATH={quoted}:$PATH ;; esac")
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn fish_path_line(dir: &Path) -> String {
    let escaped = dir
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    format!("fish_add_path --path --prepend '{escaped}'")
}

/// Nushell config line that prepends the install path only when it is absent.
pub fn nushell_path_line(dir: &Path) -> String {
    let path = nushell_raw_string(&dir.display().to_string());
    format!("if ({path} not-in $env.PATH) {{ $env.PATH = ($env.PATH | prepend {path}) }}")
}

fn nushell_raw_string(value: &str) -> String {
    for hashes in 1.. {
        let marker = "#".repeat(hashes);
        let closing = format!("'{marker}");
        if !value.contains(&closing) {
            return format!("r{marker}'{value}'{marker}");
        }
    }
    unreachable!("a finite path always has a raw-string delimiter")
}

fn add_installer_lines(profile: &Path, path_line: &str) -> Result<bool, InstallerError> {
    let original = match fs::read_to_string(profile) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(InstallerError::io(format!("read {}", profile.display()), e)),
    };
    if original
        .lines()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|lines| lines == [PROFILE_MARKER, path_line])
    {
        return Ok(false);
    }

    if let Some(parent) = profile.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| InstallerError::io(format!("create {}", parent.display()), e))?;
    }
    let permissions = fs::metadata(profile).ok().map(|m| m.permissions());
    let mut updated = original;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    if !updated.is_empty() && !updated.ends_with("\n\n") {
        updated.push('\n');
    }
    updated.push_str(PROFILE_MARKER);
    updated.push('\n');
    updated.push_str(path_line);
    updated.push('\n');
    fs::write(profile, updated)
        .map_err(|e| InstallerError::io(format!("write {}", profile.display()), e))?;
    if let Some(permissions) = permissions {
        fs::set_permissions(profile, permissions)
            .map_err(|e| InstallerError::io(format!("set permissions {}", profile.display()), e))?;
    }
    Ok(true)
}

#[cfg(windows)]
fn remove_from_registry(dir: &Path) -> Result<bool, InstallerError> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| InstallerError::Registry(format!("open HKCU\\Environment: {e}")))?;
    let raw = match key.get_raw_value("Path") {
        Ok(raw) => raw,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(InstallerError::Registry(format!("read user PATH: {e}"))),
    };
    let units: Vec<u16> = raw
        .bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    let value = String::from_utf16_lossy(&units);
    let Some(updated) = drop_path_entry(&value, dir, true) else {
        return Ok(false);
    };
    // Keep the value type: a user PATH is usually REG_EXPAND_SZ.
    let encoded: Vec<u8> = updated
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    key.set_raw_value(
        "Path",
        &winreg::RegValue {
            bytes: encoded,
            vtype: raw.vtype,
        },
    )
    .map_err(|e| InstallerError::Registry(format!("write user PATH: {e}")))?;
    Ok(true)
}

#[cfg(windows)]
fn add_to_registry(dir: &Path) -> Result<bool, InstallerError> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
        .map_err(|e| InstallerError::Registry(format!("open HKCU\\Environment: {e}")))?;
    let raw = match key.get_raw_value("Path") {
        Ok(raw) => raw,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            key.set_value("Path", &dir.display().to_string())
                .map_err(|e| InstallerError::Registry(format!("write user PATH: {e}")))?;
            return Ok(true);
        }
        Err(e) => return Err(InstallerError::Registry(format!("read user PATH: {e}"))),
    };
    let units: Vec<u16> = raw
        .bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    let value = String::from_utf16_lossy(&units);
    let target = dir.components().collect::<PathBuf>();
    let mut entries = value.split(';');
    if entries
        .next()
        .is_some_and(|entry| same_path_entry(entry, &target, true))
    {
        return Ok(false);
    }
    // Put the managed install directory first so another stale `plaesy.exe` earlier in PATH
    // cannot silently shadow the binary this installer just installed.
    let without_duplicates = drop_path_entry(&value, dir, true).unwrap_or(value);
    let updated = if without_duplicates.trim().is_empty() {
        dir.display().to_string()
    } else {
        format!("{};{}", dir.display(), without_duplicates.trim_matches(';'))
    };
    let encoded: Vec<u8> = updated
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    key.set_raw_value(
        "Path",
        &winreg::RegValue {
            bytes: encoded,
            vtype: raw.vtype,
        },
    )
    .map_err(|e| InstallerError::Registry(format!("write user PATH: {e}")))?;
    Ok(true)
}

#[cfg(not(windows))]
fn remove_from_registry(_dir: &Path) -> Result<bool, InstallerError> {
    Ok(false)
}

#[cfg(not(windows))]
fn add_to_registry(_dir: &Path) -> Result<bool, InstallerError> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_entry_removes_only_the_matching_segment() {
        let value = "/usr/bin;/home/me/.local/bin;/opt";
        let updated = drop_path_entry(value, Path::new("/home/me/.local/bin"), false);
        assert_eq!(updated.as_deref(), Some("/usr/bin;/opt"));
        assert_eq!(drop_path_entry(value, Path::new("/nowhere"), false), None);
    }

    #[test]
    fn drop_entry_folds_case_on_windows_style_paths() {
        let value = r"C:\Tools;C:\Users\Me\AppData\Local\Plaesy\bin";
        let updated = drop_path_entry(
            value,
            Path::new(r"c:\users\me\appdata\local\plaesy\bin"),
            true,
        );
        assert_eq!(updated.as_deref(), Some(r"C:\Tools"));
    }

    #[test]
    fn installer_block_is_removed_with_its_blank_line() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join(".bashrc");
        let bin = Path::new("/home/me/.local/bin");
        fs::write(
            &profile,
            format!(
                "alias ll='ls -l'\n\n{PROFILE_MARKER}\nexport PATH=\"{}:$PATH\"\n",
                bin.display()
            ),
        )
        .unwrap();
        assert!(remove_installer_lines(&profile, bin).unwrap());
        assert_eq!(fs::read_to_string(&profile).unwrap(), "alias ll='ls -l'\n");
        assert!(!remove_installer_lines(&profile, bin).unwrap());
    }

    #[test]
    fn guarded_shell_and_fish_path_blocks_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let bin = Path::new("/home/me/.local/bin");
        let guarded = format!(
            "{PROFILE_MARKER}\ncase \":$PATH:\" in *:\"{}\":*) ;; *) export PATH=\"{}\":$PATH ;; esac\n",
            bin.display(),
            bin.display()
        );
        let bashrc = dir.path().join(".bashrc");
        fs::write(&bashrc, &guarded).unwrap();
        assert!(remove_installer_lines(&bashrc, bin).unwrap());
        assert_eq!(fs::read_to_string(&bashrc).unwrap(), "");

        let fish = dir.path().join("config.fish");
        fs::write(
            &fish,
            format!(
                "{PROFILE_MARKER}\nfish_add_path --path --prepend '{}'\n",
                bin.display()
            ),
        )
        .unwrap();
        assert!(remove_installer_lines(&fish, bin).unwrap());
        assert_eq!(fs::read_to_string(&fish).unwrap(), "");
    }
}
