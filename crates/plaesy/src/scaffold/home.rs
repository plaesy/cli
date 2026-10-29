//! Finding the Plaesy source home that `init` copies from.

use std::env;
use std::path::{Path, PathBuf};

use crate::assets;
use crate::common::log::Logger;
use crate::installer;

use super::ScaffoldError;

/// Reports whether `dir` is a usable Plaesy home: it has the skill assets folder
/// and a `configs/platform.json`. A partial tree without the config would make
/// every `--ai` choice fail with an empty platform list.
#[must_use]
pub fn is_home(dir: &Path) -> bool {
    dir.join("skills").join("plaesy").join("assets").is_dir()
        && dir.join("configs").join("platform.json").is_file()
}

/// Resolves the Plaesy home.
///
/// Order: the explicit override, then `PLAESY_HOME`, then the directory that
/// contains the running executable (walking up), then the working directory
/// (walking up), then the managed home. The managed home is provisioned from
/// the embedded assets on first use, so `init` works on a machine where
/// `install` never ran.
///
/// # Errors
///
/// [`ScaffoldError::Home`] when no candidate is a usable home.
pub fn find_home(override_path: Option<&Path>, log: &Logger) -> Result<PathBuf, ScaffoldError> {
    if let Some(path) = override_path {
        if is_home(path) {
            return Ok(path.to_path_buf());
        }
        return Err(ScaffoldError::Home(format!(
            "--plaesy-home {} does not look like a Plaesy repo root (missing skills/plaesy/assets/ or configs/platform.json)",
            path.display()
        )));
    }

    if let Some(env_home) = env::var_os("PLAESY_HOME").map(PathBuf::from) {
        if is_home(&env_home) {
            return Ok(env_home);
        }
    }

    if let Some(exe_dir) = env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok().or(Some(exe)))
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        if let Some(home) = walk_up_for_home(&exe_dir) {
            return Ok(home);
        }
    }

    if let Some(home) = env::current_dir()
        .ok()
        .and_then(|cwd| walk_up_for_home(&cwd))
    {
        return Ok(home);
    }

    // Provision the managed home here, so every route to the binary yields a
    // working `init`, whether or not `install` ran.
    if let Ok(managed) = installer::plaesy_home_dir() {
        if !is_home(&managed) {
            if let Err(e) = assets::extract(&managed, log) {
                log.debug(&format!("could not provision managed home: {e}"));
            }
        }
        if is_home(&managed) {
            return Ok(managed);
        }
    }

    Err(ScaffoldError::Home(
        "could not locate or provision a Plaesy home: set PLAESY_HOME or pass --plaesy-home".into(),
    ))
}

fn walk_up_for_home(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| is_home(dir))
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_home(dir: &Path) {
        std::fs::create_dir_all(dir.join("skills/plaesy/assets")).unwrap();
        std::fs::create_dir_all(dir.join("configs")).unwrap();
        std::fs::write(dir.join("configs/platform.json"), "{}").unwrap();
    }

    #[test]
    fn a_home_needs_both_markers() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_home(dir.path()));
        fake_home(dir.path());
        assert!(is_home(dir.path()));
    }

    #[test]
    fn explicit_override_must_be_a_home() {
        let dir = tempfile::tempdir().unwrap();
        let log = Logger::new("test");
        assert!(matches!(
            find_home(Some(dir.path()), &log),
            Err(ScaffoldError::Home(_))
        ));
        fake_home(dir.path());
        assert_eq!(find_home(Some(dir.path()), &log).unwrap(), dir.path());
    }
}
