//! `configs/platform.json`: AI platforms, their mappings, and the `.plaesy` layout.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::ConfigError;

/// One entry in `plaesy.mapping`. Written either as a bare string or as an
/// object with `value`, `excludes`, and `description`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "MappingRepr")]
pub struct MappingEntry {
    pub value: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excludes: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MappingRepr {
    Plain(String),
    Full(MappingFull),
}

#[derive(Deserialize)]
struct MappingFull {
    #[serde(default)]
    value: String,
    #[serde(default)]
    excludes: Vec<String>,
    #[serde(default)]
    description: String,
}

impl From<MappingRepr> for MappingEntry {
    fn from(repr: MappingRepr) -> Self {
        match repr {
            MappingRepr::Plain(value) => Self {
                value,
                ..Self::default()
            },
            MappingRepr::Full(full) => Self {
                value: full.value,
                excludes: full.excludes,
                description: full.description,
            },
        }
    }
}

/// The `plaesy` section: base directory layout and the source-to-target mapping.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaesyStructure {
    pub base_directory: String,
    pub core_directories: Vec<String>,
    pub project_directories: Vec<String>,
    pub mapping: BTreeMap<String, MappingEntry>,
}

/// One entry under `platforms`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Platform {
    pub name: String,
    pub provider: String,
    pub category: String,
    pub detection: Vec<String>,
    pub mapping: BTreeMap<String, String>,
}

/// The whole of `platform.json`.
///
/// `platforms` is an [`IndexMap`], so iteration follows the order the file
/// declares. Detection depends on that order: the first match wins.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlatformConfig {
    pub version: String,
    pub description: String,
    pub plaesy: PlaesyStructure,
    pub platforms: IndexMap<String, Platform>,
}

/// Shorthand names people type, mapped to the ids `platform.json` declares.
///
/// Every right-hand side must be a key in `platform.json`. Long forms such as
/// `claude_code` stay here so older configs and scripts keep resolving.
const ALIASES: &[(&str, &str)] = &[
    ("claude", "claude"),
    ("claude_code", "claude"),
    ("anthropic", "claude"),
    ("copilot", "github_copilot"),
    ("github", "github_copilot"),
    ("gh", "github_copilot"),
    ("gh_copilot", "github_copilot"),
    ("cursor", "cursor_ai"),
    ("windsurf", "windsurf_ai"),
    ("continue", "continue_dev"),
    ("kilo", "kilo"),
    ("kilo_code", "kilo"),
    ("generic", "generic_ai"),
    ("none", ""),
];

/// Reports whether `name` (already lowercased, dashes as underscores) is a known alias.
#[must_use]
pub fn is_known_alias(name: &str) -> bool {
    ALIASES.iter().any(|(alias, _)| *alias == name)
}

/// Resolves a user-supplied platform name to a platform id.
///
/// Order: alias table, then a configured id (case-insensitive), then a
/// configured display name (case-insensitive), then the trimmed input unchanged
/// so the caller can report it as unknown. Empty input and `none` return `""`,
/// which means "no platform".
#[must_use]
pub fn normalize_platform(input: &str, config: Option<&PlatformConfig>) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lowered = trimmed.to_lowercase();
    if let Some((_, id)) = ALIASES.iter().find(|(alias, _)| *alias == lowered) {
        return (*id).to_owned();
    }
    if let Some(config) = config {
        for (id, platform) in &config.platforms {
            if id.eq_ignore_ascii_case(trimmed) || platform.name.eq_ignore_ascii_case(trimmed) {
                return id.clone();
            }
        }
    }
    trimmed.to_owned()
}

/// Cleanup targets used when the registry does not declare any.
#[must_use]
pub fn clean_files() -> &'static [&'static str] {
    &[
        "CLAUDE.md",
        ".cursorrules",
        ".github/copilot-instructions.md",
    ]
}

/// Globs used when copying source files, keyed by file type.
#[must_use]
pub fn source_rules(file_type: &str) -> &'static [&'static str] {
    match file_type {
        "core" => &["instructions/plaesy.instructions.md"],
        "instructions" => &["instructions/*.instructions.md", "plaesy.instructions.md"],
        "prompts" => &["prompts/*.prompt.md"],
        "agents" => &["agents/*.agents.md"],
        _ => &[],
    }
}

impl PlatformConfig {
    /// Reads and parses `platform.json` at `path`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NotFound`] when the file is absent, [`ConfigError::Json`]
    /// when it does not parse, [`ConfigError::Io`] for other read failures.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => ConfigError::NotFound(path.to_path_buf()),
            _ => ConfigError::io(format!("reading {}", path.display()), e),
        })?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Platform ids in declaration order.
    #[must_use]
    pub fn platform_ids(&self) -> Vec<&str> {
        self.platforms.keys().map(String::as_str).collect()
    }

    /// Platform ids sorted alphabetically, for stable error messages.
    #[must_use]
    pub fn sorted_platform_ids(&self) -> Vec<&str> {
        let mut ids = self.platform_ids();
        ids.sort_unstable();
        ids
    }

    /// Returns the platform with `id`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownPlatform`] when `id` is not declared.
    pub fn platform(&self, id: &str) -> Result<&Platform, ConfigError> {
        self.platforms
            .get(id)
            .ok_or_else(|| ConfigError::UnknownPlatform(id.to_owned()))
    }

    /// Reports whether `id` is a declared platform id. Pair with [`normalize_platform`].
    #[must_use]
    pub fn has_platform(&self, id: &str) -> bool {
        self.platforms.contains_key(id)
    }

    /// Reads `name`, `provider`, `category`, or `mapping.<type>` for a platform.
    /// Unknown keys return an empty string.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownPlatform`] when `id` is not declared.
    pub fn platform_field(&self, id: &str, key: &str) -> Result<String, ConfigError> {
        let platform = self.platform(id)?;
        if let Some(mapping_type) = key.strip_prefix("mapping.") {
            return Ok(platform
                .mapping
                .get(mapping_type)
                .cloned()
                .unwrap_or_default());
        }
        Ok(match key {
            "name" => platform.name.clone(),
            "provider" => platform.provider.clone(),
            "category" => platform.category.clone(),
            _ => String::new(),
        })
    }

    /// Display name for a platform id, or the id itself when unknown.
    #[must_use]
    pub fn display_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.platforms.get(id).map_or(id, |p| {
            if p.name.is_empty() {
                id
            } else {
                p.name.as_str()
            }
        })
    }

    /// Target value for a `plaesy.mapping` section, or `""` when absent.
    #[must_use]
    pub fn mapping_value(&self, section: &str) -> &str {
        self.plaesy
            .mapping
            .get(section)
            .map_or("", |entry| entry.value.as_str())
    }

    /// Exclude patterns for a `plaesy.mapping` section.
    #[must_use]
    pub fn mapping_excludes(&self, section: &str) -> &[String] {
        self.plaesy
            .mapping
            .get(section)
            .map_or(&[], |entry| entry.excludes.as_slice())
    }

    /// First platform, in declaration order, whose detection path exists relative
    /// to the current directory. `None` when nothing matches.
    #[must_use]
    pub fn detect_platform(&self) -> Option<&str> {
        self.platforms.iter().find_map(|(id, platform)| {
            platform
                .detection
                .iter()
                .any(|pattern| Path::new(pattern).exists())
                .then_some(id.as_str())
        })
    }

    /// Directories to clean. With a platform name, uses that platform's core,
    /// prompts, and agents targets. Without one, or when those yield nothing,
    /// falls back to every platform's core directory.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownPlatform`] for an undeclared name, and
    /// [`ConfigError::NoCleanDirectories`] when nothing is found.
    pub fn clean_dirs(&self, platform: Option<&str>) -> Result<Vec<String>, ConfigError> {
        let mut dirs = Vec::new();
        if let Some(id) = platform.filter(|p| !p.is_empty()) {
            let p = self.platform(id)?;
            for key in ["core", "prompts", "agents"] {
                push_dir(&mut dirs, p.mapping.get(key).map(String::as_str));
            }
        }
        if dirs.is_empty() {
            for p in self.platforms.values() {
                push_dir(&mut dirs, p.mapping.get("core").map(String::as_str));
            }
        }
        if dirs.is_empty() {
            return Err(ConfigError::NoCleanDirectories);
        }
        Ok(dirs)
    }

    /// Resolves a structure component to its value.
    ///
    /// Array components are space-joined. `memory_subdirectories` is reserved
    /// and currently always empty.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownComponent`] for any other name.
    pub fn structure_component(&self, component: &str) -> Result<String, ConfigError> {
        Ok(match component {
            "core_directories" => self.plaesy.core_directories.join(" "),
            "project_directories" => self.plaesy.project_directories.join(" "),
            "memory_subdirectories" => String::new(),
            "base_directory" => self.plaesy.base_directory.clone(),
            other => return Err(ConfigError::UnknownComponent(other.to_owned())),
        })
    }
}

/// Adds the parent directory of `target` to `dirs`, skipping empty, `null`,
/// root-level, and already-present entries.
fn push_dir(dirs: &mut Vec<String>, target: Option<&str>) {
    let Some(target) = target.filter(|t| !t.is_empty() && *t != "null") else {
        return;
    };
    let normalized = target.replace('\\', "/");
    let dir = match normalized.rsplit_once('/') {
        Some(("", _)) => "/".to_owned(),
        Some((dir, _)) => dir.to_owned(),
        None => return,
    };
    if !dirs.contains(&dir) {
        dirs.push(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "version": "1.0",
        "plaesy": {
            "base_directory": ".plaesy",
            "core_directories": ["memory", "instructions"],
            "project_directories": ["analysis"],
            "mapping": {
                "core": {"value": "instructions/plaesy.md", "excludes": ["*.draft.md"]},
                "prompts": "prompts"
            }
        },
        "platforms": {
            "claude": {
                "name": "Claude Code",
                "provider": "Anthropic",
                "category": "cli",
                "detection": ["CLAUDE.md", ".claude"],
                "mapping": {"core": "CLAUDE.md", "prompts": ".claude/commands", "agents": ".claude/agents"}
            },
            "cursor_ai": {
                "name": "Cursor",
                "provider": "Anysphere",
                "category": "ide",
                "detection": [".cursorrules"],
                "mapping": {"core": ".cursorrules"}
            }
        }
    }"#;

    fn fixture() -> PlatformConfig {
        serde_json::from_str(FIXTURE).unwrap()
    }

    #[test]
    fn mapping_entry_accepts_string_and_object_shapes() {
        let plain: MappingEntry = serde_json::from_str(r#""CLAUDE.md""#).unwrap();
        assert_eq!(plain.value, "CLAUDE.md");
        assert!(plain.excludes.is_empty());

        let full: MappingEntry =
            serde_json::from_str(r#"{"value":"x.md","excludes":["a"]}"#).unwrap();
        assert_eq!(full.value, "x.md");
        assert_eq!(full.excludes, ["a"]);
    }

    #[test]
    fn platforms_keep_declaration_order() {
        assert_eq!(fixture().platform_ids(), ["claude", "cursor_ai"]);
        assert_eq!(fixture().sorted_platform_ids(), ["claude", "cursor_ai"]);
    }

    #[test]
    fn alias_resolves_to_a_declared_id() {
        let cfg = fixture();
        assert_eq!(normalize_platform("claude_code", Some(&cfg)), "claude");
        assert_eq!(normalize_platform("Cursor", Some(&cfg)), "cursor_ai");
        assert_eq!(normalize_platform("Claude Code", Some(&cfg)), "claude");
        assert_eq!(normalize_platform("  ", Some(&cfg)), "");
        assert_eq!(normalize_platform("none", Some(&cfg)), "");
        assert_eq!(
            normalize_platform("unknownthing", Some(&cfg)),
            "unknownthing"
        );
    }

    #[test]
    fn every_alias_target_is_a_real_platform_in_the_shipped_registry() {
        let shipped = PlatformConfig::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config/platform.json"),
        )
        .unwrap();
        for (alias, target) in ALIASES {
            if !target.is_empty() {
                assert!(
                    shipped.has_platform(target),
                    "alias {alias} -> missing {target}"
                );
            }
        }
    }

    #[test]
    fn field_lookup_handles_top_level_and_mapping_keys() {
        let cfg = fixture();
        assert_eq!(cfg.platform_field("claude", "name").unwrap(), "Claude Code");
        assert_eq!(
            cfg.platform_field("claude", "mapping.agents").unwrap(),
            ".claude/agents"
        );
        assert_eq!(cfg.platform_field("claude", "bogus").unwrap(), "");
        assert!(matches!(
            cfg.platform_field("nope", "name"),
            Err(ConfigError::UnknownPlatform(_))
        ));
    }

    #[test]
    fn plaesy_mapping_value_and_excludes() {
        let cfg = fixture();
        assert_eq!(cfg.mapping_value("core"), "instructions/plaesy.md");
        assert_eq!(cfg.mapping_value("prompts"), "prompts");
        assert_eq!(cfg.mapping_value("missing"), "");
        assert_eq!(cfg.mapping_excludes("core"), ["*.draft.md"]);
        assert!(cfg.mapping_excludes("prompts").is_empty());
    }

    #[test]
    fn clean_dirs_for_a_named_platform_collects_its_targets() {
        // claude: core CLAUDE.md is root-level (skipped); prompts and agents both
        // live under .claude, which is listed once.
        assert_eq!(fixture().clean_dirs(Some("claude")).unwrap(), [".claude"]);
    }

    #[test]
    fn clean_dirs_without_a_name_uses_every_core_directory() {
        // Both fixture cores are root-level, so the fallback finds nothing.
        assert!(matches!(
            fixture().clean_dirs(None),
            Err(ConfigError::NoCleanDirectories)
        ));

        let mut cfg = fixture();
        cfg.platforms
            .get_mut("cursor_ai")
            .unwrap()
            .mapping
            .insert("core".into(), ".cursor/rules/plaesy.md".into());
        assert_eq!(cfg.clean_dirs(None).unwrap(), [".cursor/rules"]);
    }

    #[test]
    fn clean_dirs_rejects_unknown_platform() {
        assert!(matches!(
            fixture().clean_dirs(Some("nope")),
            Err(ConfigError::UnknownPlatform(_))
        ));
    }

    #[test]
    fn structure_component_joins_arrays_and_rejects_unknown() {
        let cfg = fixture();
        assert_eq!(
            cfg.structure_component("core_directories").unwrap(),
            "memory instructions"
        );
        assert_eq!(
            cfg.structure_component("base_directory").unwrap(),
            ".plaesy"
        );
        assert_eq!(
            cfg.structure_component("memory_subdirectories").unwrap(),
            ""
        );
        assert!(matches!(
            cfg.structure_component("nope"),
            Err(ConfigError::UnknownComponent(_))
        ));
    }

    #[test]
    fn push_dir_keeps_only_real_parent_directories() {
        let mut dirs = Vec::new();
        push_dir(&mut dirs, Some("CLAUDE.md"));
        push_dir(&mut dirs, Some(".claude/agents/x.md"));
        push_dir(&mut dirs, Some(".claude/agents/y.md"));
        push_dir(&mut dirs, Some("null"));
        push_dir(&mut dirs, Some(""));
        push_dir(&mut dirs, Some("/abs/file"));
        assert_eq!(dirs, [".claude/agents", "/abs"]);
    }

    #[test]
    fn load_reports_missing_file_and_bad_json() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            PlatformConfig::load(&dir.path().join("none.json")),
            Err(ConfigError::NotFound(_))
        ));
        let bad = dir.path().join("bad.json");
        fs::write(&bad, "{not json").unwrap();
        assert!(matches!(
            PlatformConfig::load(&bad),
            Err(ConfigError::Json(_))
        ));
    }
}
