//! The Plaesy source tree, embedded in the binary.
//!
//! `plaesy install` extracts it into a Plaesy home, so `plaesy init` works on a
//! machine that never cloned spec-kit. The Go version kept a generated copy
//! under `data/`. Here the directories are embedded straight from the repo
//! root, so no sync step can drift.
//!
//! Port of `scripts/internal/assets` (Go).

use std::fs;
use std::io;
use std::path::Path;

use include_dir::{include_dir, Dir};

use crate::common::log::Logger;

/// Source directories and where each one lands inside a Plaesy home.
static TREE: [(&str, Dir<'static>); 4] = [
    (
        "agents",
        include_dir!("$CARGO_MANIFEST_DIR/../../spec-kit/agents"),
    ),
    (
        "checklists",
        include_dir!("$CARGO_MANIFEST_DIR/../../spec-kit/checklists"),
    ),
    (
        "skills",
        include_dir!("$CARGO_MANIFEST_DIR/../../spec-kit/skills"),
    ),
    (
        "configs",
        include_dir!("$CARGO_MANIFEST_DIR/src/config"),
    ),
];

/// Errors while extracting the tree.
#[derive(Debug, thiserror::Error)]
pub enum AssetsError {
    #[error("writing {}: {source}", path.display())]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Writes the embedded tree into `dest`, overwriting existing files.
///
/// `dest` is a directory Plaesy owns, so nothing user-authored lives there.
/// Overwriting on every install keeps it current after an upgrade.
///
/// # Errors
///
/// [`AssetsError::Write`] for the first file or directory that cannot be written.
pub fn extract(dest: &Path, log: &Logger) -> Result<usize, AssetsError> {
    let mut written = 0;
    for (name, dir) in &TREE {
        let root = dest.join(name);
        write_dir(&root, dir, &mut written)?;
    }
    log.debug(&format!(
        "extracted {written} embedded file(s) into {}",
        dest.display()
    ));
    Ok(written)
}

fn write_dir(target: &Path, dir: &Dir<'_>, written: &mut usize) -> Result<(), AssetsError> {
    fs::create_dir_all(target).map_err(|source| AssetsError::Write {
        path: target.to_path_buf(),
        source,
    })?;
    for file in dir.files() {
        let Some(name) = file.path().file_name() else {
            continue;
        };
        let path = target.join(name);
        fs::write(&path, file.contents()).map_err(|source| AssetsError::Write {
            path: path.clone(),
            source,
        })?;
        *written += 1;
    }
    for sub in dir.dirs() {
        let Some(name) = sub.path().file_name() else {
            continue;
        };
        write_dir(&target.join(name), sub, written)?;
    }
    Ok(())
}

/// Returns the embedded bytes of `relative` inside the named source tree, or `None`.
#[must_use]
pub fn read(tree: &str, relative: &str) -> Option<&'static [u8]> {
    let (_, dir) = TREE.iter().find(|(name, _)| *name == tree)?;
    dir.get_file(relative).map(include_dir::File::contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_writes_the_home_marker_and_platform_config() {
        let dest = tempfile::tempdir().unwrap();
        let log = Logger::new("test");
        let written = extract(dest.path(), &log).unwrap();
        assert!(written > 0);
        assert!(dest
            .path()
            .join("skills/plaesy/assets/mapping.json")
            .is_file());
        assert!(dest.path().join("configs/platform.json").is_file());
    }

    #[test]
    fn extract_overwrites_existing_files() {
        let dest = tempfile::tempdir().unwrap();
        let log = Logger::new("test");
        let stale = dest.path().join("configs/platform.json");
        fs::create_dir_all(stale.parent().unwrap()).unwrap();
        fs::write(&stale, "stale").unwrap();
        extract(dest.path(), &log).unwrap();
        assert_ne!(fs::read_to_string(&stale).unwrap(), "stale");
    }

    #[test]
    fn embedded_platform_config_is_readable() {
        assert!(read("configs", "platform.json").is_some());
        assert!(read("configs", "does-not-exist.json").is_none());
    }
}
