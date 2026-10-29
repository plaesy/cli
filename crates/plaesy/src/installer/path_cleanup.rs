//! Removes the PATH entries that the installers added.
//!
//! Unix: the `export PATH=` block in the shell profiles, identified by its marker
//! line. Windows: the user `Path` value in `HKCU\Environment`. The machine PATH is
//! never touched.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::InstallerError;

/// Comment line the installer writes above its export. Removal matches it and the export after it.
pub const PROFILE_MARKER: &str = "# Added by the plaesy installer";

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

fn same_path_entry(entry: &str, target: &Path, fold: bool) -> bool {
    let candidate = entry.trim().trim_matches('"');
    if candidate.is_empty() {
        return false;
    }
    let candidate: PathBuf = Path::new(candidate).components().collect();
    if fold {
        candidate.to_string_lossy().to_lowercase() == target.to_string_lossy().to_lowercase()
    } else {
        candidate == target
    }
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
    let want = format!("export PATH=\"{}:$PATH\"", dir.display());
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut changed = false;
    let mut i = 0;
    while i < lines.len() {
        if lines[i] == PROFILE_MARKER && lines.get(i + 1) == Some(&want.as_str()) {
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
    let mut files = vec![home.join(".bashrc"), home.join(".zshrc")];
    if cfg!(target_os = "macos") {
        files.push(home.join(".bash_profile"));
    }
    Ok(files)
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
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
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

#[cfg(not(windows))]
fn remove_from_registry(_dir: &Path) -> Result<bool, InstallerError> {
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
}
