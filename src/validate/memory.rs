//! Self-containment check of `.plaesy/memory`.
//!
//! Memory notes must not depend on paths outside the project. Each note is
//! scanned for references to the user's home, `/tmp`, or a tool directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use super::ValidateError;

/// External-path patterns, as written in the source document.
pub const PATTERN_SOURCES: [&str; 7] = [
    "~/.claude",
    "~/.claude/projects",
    "~/.claude/plans",
    "/tmp/",
    r"C:\\Users\\.*\\.claude",
    "/Users/.*/.claude",
    "CLAUDE_CODE_DIR",
];

const HOME_PATH_SOURCE: &str = "absolute user-home path";
const HOME_PATH_PATTERN: &str = r#"(?i)(?:^|[\s"'`(=:])(?:~[/\\]|/(?:Users|home)/[^/\\\s]+(?:/|$)|/root(?:/|$)|[A-Z]:\\(?:Users\\[^\\]+|home\\[^\\]+|root)(?:\\|$))"#;

/// One file that holds external references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub file: PathBuf,
    /// Each matched pattern with the `line:content` entries it matched.
    pub matches: Vec<(&'static str, Vec<String>)>,
}

/// Findings for the memory folder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryReport {
    pub files_checked: usize,
    pub issues: Vec<Issue>,
}

/// Scans every `.md` file under `<repo_root>/.plaesy/memory`.
///
/// # Errors
///
/// [`ValidateError::Invalid`] when the memory folder does not exist.
pub fn validate(repo_root: &Path) -> Result<MemoryReport, ValidateError> {
    let dir = repo_root.join(".plaesy").join("memory");
    if !dir.is_dir() {
        return Err(ValidateError::invalid(format!(
            "memory directory not found: {}",
            dir.display()
        )));
    }
    let mut files = Vec::new();
    collect_markdown(&dir, &mut files)?;
    files.sort();

    let patterns = patterns();
    let mut report = MemoryReport::default();
    for file in files {
        report.files_checked += 1;
        // An unreadable note is skipped, as the bash script tolerates it.
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let mut matches = Vec::new();
        let mut matched_line_indexes = std::collections::BTreeSet::new();
        for (source, pattern) in PATTERN_SOURCES
            .iter()
            .copied()
            .chain(std::iter::once(HOME_PATH_SOURCE))
            .zip(patterns.iter())
        {
            let lines: Vec<String> = text
                .lines()
                .enumerate()
                .filter_map(|(i, line)| {
                    if !pattern.is_match(line)
                        || (source == HOME_PATH_SOURCE && matched_line_indexes.contains(&i))
                    {
                        return None;
                    }
                    matched_line_indexes.insert(i);
                    Some(format!("{}:{line}", i + 1))
                })
                .collect();
            if !lines.is_empty() {
                matches.push((source, lines));
            }
        }
        if !matches.is_empty() {
            report.issues.push(Issue { file, matches });
        }
    }
    Ok(report)
}

fn collect_markdown(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ValidateError> {
    let entries = fs::read_dir(dir)
        .map_err(|e| ValidateError::io(format!("reading {}", dir.display()), e))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| ValidateError::io(format!("reading {}", dir.display()), e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    Ok(())
}

fn patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        PATTERN_SOURCES
            .iter()
            .map(|p| Regex::new(p).expect("static pattern"))
            .chain(std::iter::once(
                Regex::new(HOME_PATH_PATTERN).expect("static home-path pattern"),
            ))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_external_paths_and_passes_self_contained_notes() {
        let dir = tempfile::tempdir().unwrap();
        let memory = dir.path().join(".plaesy/memory");
        fs::create_dir_all(memory.join("sub")).unwrap();
        fs::write(memory.join("bad.md"), "ok\nsee ~/.claude/plans/x\n").unwrap();
        fs::write(memory.join("sub/good.md"), "see docs/x.md\n").unwrap();

        let report = validate(dir.path()).unwrap();
        assert_eq!(report.files_checked, 2);
        assert_eq!(report.issues.len(), 1);
        let issue = &report.issues[0];
        assert!(issue.file.ends_with("bad.md"));
        let patterns: Vec<_> = issue.matches.iter().map(|(p, _)| *p).collect();
        assert_eq!(patterns, ["~/.claude", "~/.claude/plans"]);
    }

    #[test]
    fn missing_memory_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(validate(dir.path()).is_err());
    }
}
