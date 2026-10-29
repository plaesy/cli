//! Audit of `ASSUMED — <default>` labels in the Markdown corpus.
//!
//! An autonomous run records every default it applied instead of stopping to
//! ask. This check is an audit, not a gate. It fails only when `review_days` is
//! positive, and then a tag is stale when its file has had no commit within that
//! window. Age is measured per file, because a per-line blame breaks on any
//! unrelated edit to the same file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;

use crate::exclude::Ignore;

use super::ValidateError;

/// One `ASSUMED` label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub file: PathBuf,
    pub line: usize,
    pub text: String,
}

/// Result of the scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssumptionsReport {
    pub files_scanned: usize,
    pub tags: Vec<Tag>,
    /// The configured window. `0` means informational only.
    pub review_days: u32,
    /// Tags whose file is older than the window. Empty when `review_days` is `0`.
    pub stale: Vec<Tag>,
}

/// Scans every Markdown file under `repo_root` for `ASSUMED` tags.
///
/// # Errors
///
/// [`ValidateError::Io`] when the tree cannot be walked.
pub fn scan(repo_root: &Path, review_days: u32) -> Result<AssumptionsReport, ValidateError> {
    let ignore = Ignore::load(repo_root);
    let mut files = Vec::new();
    walk(repo_root, repo_root, &ignore, &mut files)?;
    files.sort();

    let mut report = AssumptionsReport {
        review_days,
        ..AssumptionsReport::default()
    };
    for file in files {
        report.files_scanned += 1;
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let file_tags: Vec<Tag> = text
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                let caps = tag_pattern().captures(line)?;
                Some(Tag {
                    file: file.clone(),
                    line: i + 1,
                    text: caps[1].trim().to_owned(),
                })
            })
            .collect();
        if file_tags.is_empty() {
            continue;
        }
        if review_days > 0 && age_days(&file).is_some_and(|age| age > f64::from(review_days)) {
            report.stale.extend(file_tags.iter().cloned());
        }
        report.tags.extend(file_tags);
    }
    Ok(report)
}

fn walk(
    root: &Path,
    dir: &Path,
    ignore: &Ignore,
    out: &mut Vec<PathBuf>,
) -> Result<(), ValidateError> {
    let entries = fs::read_dir(dir)
        .map_err(|e| ValidateError::io(format!("reading {}", dir.display()), e))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| ValidateError::io(format!("reading {}", dir.display()), e))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !ignore.skip_dir(&rel, &name) {
                walk(root, &path, ignore, out)?;
            }
        } else if !ignore.skip_file(&rel) && name.ends_with(".md") {
            out.push(path);
        }
    }
    Ok(())
}

/// Days since the file's last commit, from `git log`. `None` when the file is
/// untracked or git is unavailable. Callers then cannot judge staleness.
fn age_days(file: &Path) -> Option<f64> {
    let dir = file.parent()?;
    let name = file.file_name()?;
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["log", "-1", "--format=%ct", "--"])
        .arg(name)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let committed: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(now.saturating_sub(committed) as f64 / 86_400.0)
}

fn tag_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // A hyphen is accepted as well as the em dash, in case a hand-typed note avoids it.
    RE.get_or_init(|| Regex::new(r"ASSUMED\s*[—-]\s*(.+)").expect("static regex"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tags_with_their_lines_and_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "ignored/\n").unwrap();
        fs::create_dir_all(dir.path().join("ignored")).unwrap();
        fs::write(dir.path().join("ignored/x.md"), "ASSUMED — skip me\n").unwrap();
        fs::write(
            dir.path().join("notes.md"),
            "intro\nASSUMED — default to UTC\nASSUMED - hyphen form\n",
        )
        .unwrap();

        let report = scan(dir.path(), 0).unwrap();
        assert_eq!(report.files_scanned, 1);
        let texts: Vec<_> = report
            .tags
            .iter()
            .map(|t| (t.line, t.text.as_str()))
            .collect();
        assert_eq!(texts, [(2, "default to UTC"), (3, "hyphen form")]);
        assert!(report.stale.is_empty());
    }

    #[test]
    fn untracked_files_cannot_be_judged_stale() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "ASSUMED — x\n").unwrap();
        let report = scan(dir.path(), 1).unwrap();
        assert_eq!(report.tags.len(), 1);
        assert!(report.stale.is_empty());
    }
}
