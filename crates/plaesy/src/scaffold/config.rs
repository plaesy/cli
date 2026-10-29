//! Loading `platform.json` for scaffolding, with the defaults `init` relies on.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::platform::PlatformConfig;

use super::ScaffoldError;

/// Base directory created when `platform.json` leaves it out.
pub const DEFAULT_BASE_DIRECTORY: &str = ".plaesy";

/// Core directories created when `platform.json` leaves them out.
pub const DEFAULT_CORE_DIRECTORIES: [&str; 2] = ["memory", "instructions"];

/// Path of the `platform.json` shipped in a Plaesy home.
#[must_use]
pub fn platform_config_path(home: &Path) -> PathBuf {
    home.join("configs").join("platform.json")
}

/// Loads `platform.json` from `home` and fills in the scaffold defaults for a
/// missing base directory or core directory list.
///
/// # Errors
///
/// [`ScaffoldError::Config`] when the file is missing or does not parse.
pub fn load(home: &Path) -> Result<PlatformConfig, ScaffoldError> {
    let path = platform_config_path(home);
    let mut cfg = PlatformConfig::load(&path)
        .map_err(|e| ScaffoldError::Config(format!("{e} (reading {})", path.display())))?;
    if cfg.plaesy.base_directory.is_empty() {
        cfg.plaesy.base_directory = DEFAULT_BASE_DIRECTORY.to_owned();
    }
    if cfg.plaesy.core_directories.is_empty() {
        cfg.plaesy.core_directories = DEFAULT_CORE_DIRECTORIES.map(String::from).to_vec();
    }
    Ok(cfg)
}

/// The always-load and scope-load instruction lists from `mapping.json`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct InstructionLists {
    pub always_load: Vec<String>,
    pub scope_load: Vec<String>,
}

#[derive(Deserialize)]
struct MappingFile {
    mappings: InstructionLists,
}

/// Reads `skills/plaesy/assets/mapping.json`. This file is the single source of
/// truth for which instruction files are always loaded. A missing or corrupt
/// file is an error, so a stale copy is never silently used.
///
/// # Errors
///
/// [`ScaffoldError::Config`] when the file is missing, does not parse, or has no
/// `always_load` entries.
pub fn instruction_lists(home: &Path) -> Result<InstructionLists, ScaffoldError> {
    let path = home
        .join("skills")
        .join("plaesy")
        .join("assets")
        .join("mapping.json");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        ScaffoldError::Config(format!("read skills/plaesy/assets/mapping.json: {e}"))
    })?;
    let parsed: MappingFile = serde_json::from_str(&text).map_err(|e| {
        ScaffoldError::Config(format!("parse skills/plaesy/assets/mapping.json: {e}"))
    })?;
    if parsed.mappings.always_load.is_empty() {
        return Err(ScaffoldError::Config(
            "skills/plaesy/assets/mapping.json mappings.always_load is empty".into(),
        ));
    }
    Ok(parsed.mappings)
}

/// Installed file name for an instruction source. A skill's `SKILL.md` installs
/// under its folder name (`skills/go/SKILL.md` → `go.md`). Other files keep their name.
#[must_use]
pub fn instruction_name(rel: &str) -> String {
    let path = Path::new(rel);
    let file = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    if file == "SKILL.md" {
        if let Some(folder) = path.parent().and_then(Path::file_name) {
            return format!("{}.md", folder.to_string_lossy());
        }
    }
    file
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_entry_installs_under_its_folder_name() {
        assert_eq!(instruction_name("skills/go/SKILL.md"), "go.md");
        assert_eq!(
            instruction_name("skills/plaesy/references/tasks.md"),
            "tasks.md"
        );
    }

    #[test]
    fn missing_platform_config_is_a_config_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(load(dir.path()), Err(ScaffoldError::Config(_))));
    }

    #[test]
    fn defaults_fill_missing_base_and_core_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("configs")).unwrap();
        std::fs::write(
            dir.path().join("configs/platform.json"),
            r#"{"plaesy":{"base_directory":"","core_directories":[]},"platforms":{}}"#,
        )
        .unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.plaesy.base_directory, ".plaesy");
        assert_eq!(cfg.plaesy.core_directories, ["memory", "instructions"]);
    }
}
