//! `plaesy clean`: removes Plaesy framework files and directories from a project.
//!
//! What is removed is driven by `platform.json`. The level controls how much:
//! `safe` removes `.plaesy/` only, `thorough` removes the platform mappings too, and
//! `complete` also removes the generic AI files. `docs/` is never removed.
//! Port of `scripts/internal/cleaner` (Go).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::Local;
use thiserror::Error;

use crate::common::log::Logger;
use crate::config::platform::PlatformConfig;

/// Directories a level may remove at the top level of the project.
const PLAESY_DIRS: [&str; 1] = [".plaesy"];

/// Mapping types that the platform mapping covers, in order.
const MAPPING_TYPES: [&str; 4] = ["core", "instructions", "prompts", "agents"];

/// Errors from cleaning.
#[derive(Debug, Error)]
pub enum CleanError {
    /// The target directory is missing, unreadable, or not writable.
    #[error("{0}")]
    Target(String),
    /// Auto-detection could not read a marker, so the platform set is unknown.
    #[error("cannot read the {platform} detection marker {pattern:?}: {source}")]
    Detection {
        platform: String,
        pattern: String,
        #[source]
        source: io::Error,
    },
    /// The level name is not one of `safe`, `thorough`, `complete`.
    #[error("invalid cleanup level: {0} (must be one of: safe, thorough, complete)")]
    Level(String),
    /// Some items could not be removed.
    #[error("failed to remove {count} item(s): {items}")]
    Partial { count: usize, items: String },
    /// A path could not be inspected.
    #[error("cannot access {path}: {source}")]
    Access {
        path: String,
        #[source]
        source: io::Error,
    },
    /// Any other I/O failure, with context.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
}

/// Cleanup aggressiveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    #[default]
    Safe,
    Thorough,
    Complete,
}

impl FromStr for Level {
    type Err = CleanError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "safe" => Ok(Self::Safe),
            "thorough" => Ok(Self::Thorough),
            "complete" => Ok(Self::Complete),
            other => Err(CleanError::Level(other.to_owned())),
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Safe => "safe",
            Self::Thorough => "thorough",
            Self::Complete => "complete",
        })
    }
}

impl Level {
    /// Top-level directories this level may remove.
    #[must_use]
    pub fn directories(self) -> &'static [&'static str] {
        match self {
            Self::Safe | Self::Thorough | Self::Complete => &PLAESY_DIRS,
        }
    }

    /// Whether the generic AI files are included. Only `complete` includes them.
    #[must_use]
    pub fn includes_generic_ai(self) -> bool {
        self == Self::Complete
    }
}

/// Options for one clean run.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub target_dir: PathBuf,
    pub auto_confirm: bool,
    pub dry_run: bool,
    pub backup: bool,
    pub level: Level,
    /// Explicit platform names, whitespace-separated. Empty means auto-detect.
    pub ai_choice: String,
}

/// What a run would remove.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub platforms: Vec<String>,
    /// Top-level Plaesy directories that exist.
    pub plaesy_dirs: Vec<String>,
    /// Per platform, the existing targets with a `/` suffix for directories.
    pub platform_entries: BTreeMap<String, Vec<String>>,
}

impl Plan {
    /// Whether nothing was found to remove.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plaesy_dirs.is_empty() && self.platform_entries.is_empty()
    }
}

/// Drives a clean run against one project.
#[derive(Debug)]
pub struct Cleaner {
    pub options: Options,
    pub config: PlatformConfig,
    pub config_path: PathBuf,
}

impl Cleaner {
    /// Resolves the target directory and loads `platform.json`.
    ///
    /// # Errors
    ///
    /// [`CleanError::Io`] when the config cannot be loaded.
    pub fn new(mut options: Options, config: PlatformConfig, config_path: PathBuf) -> Self {
        if options.target_dir.as_os_str().is_empty() {
            options.target_dir = PathBuf::from(".");
        }
        if let Ok(abs) = options.target_dir.canonicalize() {
            options.target_dir = abs;
        }
        Self {
            options,
            config,
            config_path,
        }
    }

    /// Confirms the target exists, is a directory, and is readable and writable.
    ///
    /// # Errors
    ///
    /// [`CleanError::Target`] for each failure mode, named precisely.
    pub fn validate_environment(&self) -> Result<(), CleanError> {
        let target = &self.options.target_dir;
        let info = fs::metadata(target).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                CleanError::Target(format!(
                    "target directory '{}' does not exist",
                    target.display()
                ))
            } else {
                CleanError::Target(format!(
                    "cannot access target directory '{}': {e}",
                    target.display()
                ))
            }
        })?;
        if !info.is_dir() {
            return Err(CleanError::Target(format!(
                "target '{}' exists but is not a directory",
                target.display()
            )));
        }
        fs::read_dir(target).map_err(|_| {
            CleanError::Target(format!(
                "target directory '{}' is not readable",
                target.display()
            ))
        })?;
        let probe = target.join(".plaesy-clean-write-test");
        fs::File::create(&probe).map_err(|_| {
            CleanError::Target(format!(
                "target directory '{}' is not writable",
                target.display()
            ))
        })?;
        let _ = fs::remove_file(&probe);
        Ok(())
    }

    /// Platforms whose detection marker exists in the target, falling back to
    /// `generic_ai` when none match. A marker that exists but cannot be read is an
    /// error, because the answer decides which files get deleted.
    ///
    /// # Errors
    ///
    /// [`CleanError::Detection`] when a marker cannot be inspected.
    pub fn detect_all_platforms(&self) -> Result<Vec<String>, CleanError> {
        let mut found = Vec::new();
        for id in self.config.platform_ids() {
            let Some(platform) = self.config.platforms.get(id) else {
                continue;
            };
            for pattern in &platform.detection {
                match fs::metadata(self.options.target_dir.join(pattern)) {
                    Ok(_) => {
                        found.push(id.to_owned());
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(source) => {
                        return Err(CleanError::Detection {
                            platform: id.to_owned(),
                            pattern: pattern.clone(),
                            source,
                        })
                    }
                }
            }
        }
        if found.is_empty() {
            Ok(vec!["generic_ai".to_owned()])
        } else {
            Ok(found)
        }
    }

    /// The platforms to act on: the explicit choice, else auto-detection.
    ///
    /// # Errors
    ///
    /// Auto-detection errors only. An explicit choice never touches the disk.
    pub fn resolve_platforms(&self) -> Result<Vec<String>, CleanError> {
        if !self.options.ai_choice.trim().is_empty() {
            return Ok(self
                .options
                .ai_choice
                .split_whitespace()
                .map(str::to_owned)
                .collect());
        }
        let platforms = self.detect_all_platforms()?;
        Ok(if platforms.is_empty() {
            vec!["generic".to_owned()]
        } else {
            platforms
        })
    }

    /// The target path of one mapping type for a platform, or `None`.
    fn mapping_target(&self, platform: &str, mapping_type: &str) -> Option<&str> {
        let value = self
            .config
            .platforms
            .get(platform)?
            .mapping
            .get(mapping_type)?;
        (value != "null" && !value.is_empty()).then_some(value.as_str())
    }

    /// Platforms the level acts on. `generic` is never acted on, and `generic_ai`
    /// only at `complete`.
    fn acted_on(&self, platforms: Vec<String>) -> Vec<String> {
        platforms
            .into_iter()
            .filter(|p| p != "generic")
            .filter(|p| p != "generic_ai" || self.options.level.includes_generic_ai())
            .collect()
    }

    /// Works out what a run would remove. Nothing is changed.
    ///
    /// # Errors
    ///
    /// Auto-detection errors.
    pub fn plan(&self) -> Result<Plan, CleanError> {
        let platforms = self.resolve_platforms()?;
        let mut plan = Plan {
            platforms: platforms.clone(),
            ..Plan::default()
        };
        for dir in self.options.level.directories() {
            if self.options.target_dir.join(dir).is_dir() {
                plan.plaesy_dirs.push((*dir).to_owned());
            }
        }
        for platform in self.acted_on(platforms) {
            let mut entries = Vec::new();
            for mapping_type in MAPPING_TYPES {
                let Some(target) = self.mapping_target(&platform, mapping_type) else {
                    continue;
                };
                let full = self.options.target_dir.join(target);
                match fs::metadata(&full) {
                    Ok(info) if info.is_dir() => entries.push(format!("{target}/ (directory)")),
                    Ok(_) => entries.push(format!("{target} (file)")),
                    Err(_) => {}
                }
            }
            if !entries.is_empty() {
                plan.platform_entries.insert(platform, entries);
            }
        }
        Ok(plan)
    }

    /// Prints the plan.
    ///
    /// # Errors
    ///
    /// Write failures on `out`.
    pub fn print_plan(&self, plan: &Plan, out: &mut impl Write) -> io::Result<()> {
        if self.options.dry_run {
            writeln!(out, "[DRY RUN MODE] No files will be actually removed.\n")?;
        }
        writeln!(
            out,
            "Cleanup Plan (Level: {}, Platforms: {})",
            self.options.level,
            plan.platforms.join(", ")
        )?;
        writeln!(out, "{}", "=".repeat(67))?;
        match self.options.level {
            Level::Safe => writeln!(
                out,
                "Safe cleanup: Framework files only, preserve user code"
            )?,
            Level::Thorough => writeln!(out, "Thorough cleanup: Framework, preserve user code")?,
            Level::Complete => writeln!(
                out,
                "Complete cleanup: Everything Plaesy-related (DANGEROUS)"
            )?,
        }

        writeln!(out, "\nPlaesy Framework Directories:")?;
        for dir in &plan.plaesy_dirs {
            writeln!(out, "  - {dir}/")?;
        }

        let acted = self.acted_on(plan.platforms.clone());
        for platform in &acted {
            writeln!(out, "\nPlatform-Specific Files ({platform}):")?;
            writeln!(out, "  Based on platform.json mapping for {platform}:")?;
            match plan.platform_entries.get(platform) {
                Some(entries) => {
                    for entry in entries {
                        writeln!(out, "  - {entry}")?;
                    }
                }
                None => writeln!(out, "  - No platform-specific files found for {platform}")?,
            }
        }
        if acted.is_empty() {
            writeln!(out, "\nPlatform-Specific Files:")?;
            writeln!(out, "  - Generic platform: No specific files to remove")?;
        }

        writeln!(out, "\nWhat will be preserved:")?;
        writeln!(out, "  - Your source code (src/, lib/, components/, etc.)")?;
        writeln!(out, "  - Git repository (.git/)")?;
        writeln!(out, "  - User-generated files not in Plaesy directories")?;
        writeln!(out, "  - Configuration files (.env, .gitignore, etc.)")?;
        if self.options.backup && !self.options.dry_run {
            writeln!(out, "  - Backup will be created before removal")?;
        }
        writeln!(out)
    }

    /// Asks for confirmation. `auto_confirm` answers yes without reading input.
    ///
    /// # Errors
    ///
    /// Write failures on `out`.
    pub fn confirm(&self, input: &mut impl BufRead, out: &mut impl Write) -> io::Result<bool> {
        if self.options.auto_confirm {
            writeln!(out, "Auto-confirm mode: proceeding with deletion...")?;
            return Ok(true);
        }
        writeln!(
            out,
            "This will permanently delete the directories and files listed above."
        )?;
        write!(out, "Are you sure you want to continue? (y/N): ")?;
        out.flush()?;
        let mut line = String::new();
        input.read_line(&mut line)?;
        Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
    }

    /// Copies everything this run would delete into `<target>/.plaesy-backup-<time>`.
    /// Returns the backup path, or `None` when backup is off.
    ///
    /// # Errors
    ///
    /// I/O failures while copying.
    pub fn create_backup(&self, log: &Logger) -> Result<Option<PathBuf>, CleanError> {
        if !self.options.backup {
            return Ok(None);
        }
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let backup = self
            .options
            .target_dir
            .join(format!(".plaesy-backup-{stamp}"));
        log.info(&format!("Creating backup: {}", backup.display()));
        fs::create_dir_all(&backup).map_err(|e| io_err(&backup, e))?;

        for dir in self.options.level.directories() {
            let src = self.options.target_dir.join(dir);
            if src.is_dir() {
                copy_tree(&src, &backup.join(dir))?;
                log.info(&format!("Backed up Plaesy directory: {dir}"));
            }
        }

        for platform in self.acted_on(self.resolve_platforms()?) {
            for mapping_type in MAPPING_TYPES {
                let Some(target) = self.mapping_target(&platform, mapping_type) else {
                    continue;
                };
                let full = self.options.target_dir.join(target);
                let dest = backup.join(target);
                if full.is_dir() {
                    copy_tree(&full, &dest)?;
                    log.info(&format!(
                        "Backed up platform {platform} directory: {target}"
                    ));
                } else if full.is_file() {
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
                    }
                    fs::copy(&full, &dest).map_err(|e| io_err(&full, e))?;
                    log.info(&format!("Backed up platform {platform} file: {target}"));
                }
            }
        }
        log.success(&format!("Backup created: {}", backup.display()));
        Ok(Some(backup))
    }

    /// Removes the framework directories, then each acted-on platform's mapped
    /// targets, then empty parent directories of those targets. Returns the number
    /// of items removed. Per-item failures are reported, then returned as an error.
    ///
    /// # Errors
    ///
    /// [`CleanError::Partial`] when some items could not be removed, and access
    /// errors for directories that cannot be inspected.
    pub fn remove(&self, log: &Logger, out: &mut impl Write) -> Result<usize, CleanError> {
        let target = &self.options.target_dir;
        let mut removed = 0usize;
        let mut failed: Vec<String> = Vec::new();

        log.info("Removing plaesy directories based on platform.json mapping...");
        log.info("Removing Plaesy framework directories...");
        for dir in self.options.level.directories() {
            let full = target.join(dir);
            match fs::metadata(&full) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(CleanError::Access {
                        path: full.display().to_string(),
                        source,
                    })
                }
                Ok(info) if !info.is_dir() => continue,
                Ok(_) => {}
            }
            if fs::remove_dir_all(&full).is_ok() {
                removed += 1;
                let _ = writeln!(out, "  ✅ Removed Plaesy framework: {dir}");
            } else {
                failed.push((*dir).to_owned());
                let _ = writeln!(out, "  ❌ Failed to remove: {dir}");
            }
        }

        for platform in self.acted_on(self.resolve_platforms()?) {
            log.info(&format!("Removing platform-specific files for: {platform}"));
            let mut parents = BTreeSet::new();
            for mapping_type in MAPPING_TYPES {
                let Some(target_rel) = self.mapping_target(&platform, mapping_type) else {
                    continue;
                };
                let full = target.join(target_rel);
                let Ok(info) = fs::metadata(&full) else {
                    continue;
                };
                let result = if info.is_dir() {
                    fs::remove_dir_all(&full)
                } else {
                    fs::remove_file(&full)
                };
                if result.is_ok() {
                    removed += 1;
                    let _ = writeln!(
                        out,
                        "  ✅ Removed platform {platform} {mapping_type}: {target_rel}"
                    );
                } else {
                    failed.push(target_rel.to_owned());
                    let _ = writeln!(out, "  ❌ Failed to remove: {target_rel}");
                }
                if let Some(parent) = Path::new(target_rel)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                {
                    parents.insert(parent.to_string_lossy().into_owned());
                }
            }
            for parent in parents {
                let full = target.join(&parent);
                let empty = fs::read_dir(&full).is_ok_and(|mut e| e.next().is_none());
                if empty && fs::remove_dir(&full).is_ok() {
                    removed += 1;
                    let _ = writeln!(out, "  ✅ Removed empty parent directory: {parent}");
                }
            }
        }

        if removed > 0 {
            log.success(&format!("Successfully removed {removed} items"));
        } else {
            log.warning("No items were removed");
        }
        if !failed.is_empty() {
            let items = failed.join(" ");
            log.warning(&format!("Failed to remove some items: {items}"));
            return Err(CleanError::Partial {
                count: failed.len(),
                items,
            });
        }
        Ok(removed)
    }
}

fn io_err(path: &Path, source: io::Error) -> CleanError {
    CleanError::Io {
        context: format!("copying {}", path.display()),
        source,
    }
}

/// Recursively copies `src` to `dst`, creating directories as needed.
fn copy_tree(src: &Path, dst: &Path) -> Result<(), CleanError> {
    fs::create_dir_all(dst).map_err(|e| io_err(dst, e))?;
    for entry in fs::read_dir(src).map_err(|e| io_err(src, e))?.flatten() {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| io_err(&path, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::platform::Platform;

    fn config() -> PlatformConfig {
        let mut cfg = PlatformConfig::default();
        cfg.platforms.insert(
            "claude".into(),
            Platform {
                detection: vec!["CLAUDE.md".into()],
                mapping: BTreeMap::from([
                    ("core".into(), "CLAUDE.md".into()),
                    ("agents".into(), ".claude/agents".into()),
                ]),
                ..Platform::default()
            },
        );
        cfg
    }

    fn cleaner(dir: &Path, level: Level, ai: &str) -> Cleaner {
        Cleaner::new(
            Options {
                target_dir: dir.to_path_buf(),
                level,
                ai_choice: ai.into(),
                ..Options::default()
            },
            config(),
            PathBuf::from("platform.json"),
        )
    }

    #[test]
    fn levels_parse_and_reject_unknown_names() {
        assert_eq!("thorough".parse::<Level>().unwrap(), Level::Thorough);
        assert!(matches!(
            "everything".parse::<Level>(),
            Err(CleanError::Level(_))
        ));
    }

    #[test]
    fn every_level_removes_only_the_plaesy_directory() {
        for level in [Level::Safe, Level::Thorough, Level::Complete] {
            assert_eq!(level.directories(), [".plaesy"], "level {level}");
        }
    }

    #[test]
    fn detection_finds_the_claude_marker() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "x").unwrap();
        let c = cleaner(dir.path(), Level::Thorough, "");
        assert_eq!(c.detect_all_platforms().unwrap(), ["claude"]);
    }

    #[test]
    fn no_marker_falls_back_to_generic_ai() {
        let dir = tempfile::tempdir().unwrap();
        let c = cleaner(dir.path(), Level::Thorough, "");
        assert_eq!(c.detect_all_platforms().unwrap(), ["generic_ai"]);
    }

    #[test]
    fn remove_deletes_the_plaesy_directory_at_thorough() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".plaesy/memory")).unwrap();
        fs::write(dir.path().join(".plaesy/memory/notes.md"), "x").unwrap();
        fs::write(dir.path().join("keep.txt"), "mine").unwrap();
        let c = cleaner(dir.path(), Level::Thorough, "generic");
        let mut out = Vec::new();
        let removed = c.remove(&Logger::new("t"), &mut out).unwrap();
        assert_eq!(removed, 1);
        assert!(!dir.path().join(".plaesy").exists());
        assert!(
            dir.path().join("keep.txt").exists(),
            "user files are never removed"
        );
    }

    #[test]
    fn plan_lists_existing_platform_targets_with_their_kind() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "x").unwrap();
        fs::create_dir_all(dir.path().join(".claude/agents")).unwrap();
        let plan = cleaner(dir.path(), Level::Thorough, "claude")
            .plan()
            .unwrap();
        assert_eq!(
            plan.platform_entries["claude"],
            ["CLAUDE.md (file)", ".claude/agents/ (directory)"]
        );
    }

    #[test]
    fn copy_tree_copies_nested_files() {
        let src = tempfile::tempdir().unwrap();
        fs::create_dir_all(src.path().join("a/b")).unwrap();
        fs::write(src.path().join("a/b/c.txt"), "deep").unwrap();
        let dst = tempfile::tempdir().unwrap();
        copy_tree(src.path(), &dst.path().join("copy")).unwrap();
        assert_eq!(
            fs::read_to_string(dst.path().join("copy/a/b/c.txt")).unwrap(),
            "deep"
        );
    }
}
