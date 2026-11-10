//! Compression rules: `configs/rules/trim.json` with built-in defaults.
//!
//! A project override at `.plaesy/configs/rules/trim.json` wins over the framework
//! copy. Without either file, the built-in defaults are used. A file that parses but
//! leaves out a section gets that section from the defaults, not from zero.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Line cap used when no rule matches.
pub const DEFAULT_MAX_LINES: usize = 40;

/// One Layer 1 rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layer1Rule {
    pub max_lines: usize,
    pub dedupe_consecutive: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mode: String,
}

impl Default for Layer1Rule {
    fn default() -> Self {
        Self {
            max_lines: DEFAULT_MAX_LINES,
            dedupe_consecutive: true,
            mode: String::new(),
        }
    }
}

/// A Layer 2 filler pattern: a regex and its replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillerPattern {
    pub find: String,
    pub replace: String,
}

/// Token-count thresholds that choose the Layer 2 level from a file's graph degree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DegreeThresholds {
    pub lite_min_degree: u32,
    pub full_min_degree: u32,
}

/// The full rules document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub description: String,
    pub version: String,
    pub layer1_commands: BTreeMap<String, Layer1Rule>,
    pub layer2_filler_patterns: Vec<FillerPattern>,
    pub layer2_degree_thresholds: DegreeThresholds,
}

impl Default for Rules {
    fn default() -> Self {
        let rule = |max_lines, mode: &str| Layer1Rule {
            max_lines,
            dedupe_consecutive: true,
            mode: mode.to_owned(),
        };
        let filler = |find: &str, replace: &str| FillerPattern {
            find: find.to_owned(),
            replace: replace.to_owned(),
        };
        Self {
            description: "Rules for plaesy-trim: Layer 1 (command output) thresholds and Layer 2 (prose compression) filler patterns.".into(),
            version: "1.0.0".into(),
            layer1_commands: BTreeMap::from([
                ("_default".into(), rule(DEFAULT_MAX_LINES, "")),
                ("git status".into(), rule(25, "")),
                ("git push".into(), rule(10, "")),
                ("npm install".into(), rule(15, "")),
                ("npm test".into(), rule(50, "test-results")),
                ("cargo test".into(), rule(40, "test-results")),
                ("cargo build".into(), rule(20, "")),
                ("pytest".into(), rule(50, "test-results")),
                ("dotnet build".into(), rule(25, "")),
                ("dotnet test".into(), rule(40, "test-results")),
                ("docker build".into(), rule(20, "")),
                ("git log".into(), Layer1Rule { max_lines: 30, dedupe_consecutive: false, mode: String::new() }),
            ]),
            layer2_filler_patterns: vec![
                filler(r"\bIt is important to note that\b", ""),
                filler(r"\bPlease note that\b", ""),
                filler(r"\bYou should\b", ""),
                filler(r"\bYou will need to\b", "Need to"),
                filler(r"\bin order to\b", "to"),
                filler(r"\bmake sure to\b", ""),
                filler(r"\bshould be considered as\b", "is"),
                filler(r"\bthere is a need to\b", "must"),
                filler(r"\bas well as\b", "and"),
                filler(r"\bdue to the fact that\b", "because"),
            ],
            layer2_degree_thresholds: DegreeThresholds {
                lite_min_degree: 5,
                full_min_degree: 1,
            },
        }
    }
}

/// Where the rules file lives: the project override if present, otherwise the framework copy.
#[must_use]
pub fn resolve_path(repo_root: &Path, framework_root: Option<&Path>) -> PathBuf {
    let project = repo_root
        .join(".plaesy")
        .join("configs")
        .join("rules")
        .join("trim.json");
    if is_file(&project) {
        return project;
    }
    if let Some(framework) = framework_root {
        let shipped = framework.join("configs").join("rules").join("trim.json");
        if is_file(&shipped) {
            return shipped;
        }
    }
    project
}

fn is_file(path: &Path) -> bool {
    path.is_file()
}

/// Loads the rules at `path`. A missing or unparseable file gives the defaults. A file
/// that leaves out a section gets that section from the defaults.
#[must_use]
pub fn load(path: &Path) -> Rules {
    let Ok(text) = fs::read_to_string(path) else {
        return Rules::default();
    };
    let Ok(mut rules) = serde_json::from_str::<Rules>(&text) else {
        return Rules::default();
    };
    let defaults = Rules::default();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if doc.get("layer1_commands").is_none() {
        rules.layer1_commands = defaults.layer1_commands;
    }
    if doc.get("layer2_filler_patterns").is_none() {
        rules.layer2_filler_patterns = defaults.layer2_filler_patterns;
    }
    let thresholds = doc.get("layer2_degree_thresholds");
    if thresholds.and_then(|t| t.get("lite_min_degree")).is_none() {
        rules.layer2_degree_thresholds.lite_min_degree =
            defaults.layer2_degree_thresholds.lite_min_degree;
    }
    if thresholds.and_then(|t| t.get("full_min_degree")).is_none() {
        rules.layer2_degree_thresholds.full_min_degree =
            defaults.layer2_degree_thresholds.full_min_degree;
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_partial_override_keeps_the_missing_sections_from_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trim.json");
        fs::write(&path, r#"{"version":"9","layer2_filler_patterns":[]}"#).unwrap();
        let rules = load(&path);
        assert_eq!(rules.version, "9");
        assert!(rules.layer2_filler_patterns.is_empty());
        assert_eq!(rules.layer2_degree_thresholds.lite_min_degree, 5);
        assert!(rules.layer1_commands.contains_key("cargo test"));
    }

    #[test]
    fn explicit_zero_thresholds_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trim.json");
        fs::write(
            &path,
            r#"{"layer2_degree_thresholds":{"lite_min_degree":0,"full_min_degree":0}}"#,
        )
        .unwrap();
        let rules = load(&path);
        assert_eq!(rules.layer2_degree_thresholds.lite_min_degree, 0);
    }
}
