//! Layer 2: compresses prose in memory and instruction files. Fenced code blocks
//! are left as they are.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use super::graph::resolve_level;
use super::rules::Rules;
use super::stats;

fn multi_space() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(" {2,}").expect("static regex"))
}

fn blank_run() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("\n{3,}").expect("static regex"))
}

fn stopwords() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(that|which|very|really|basically|simply)\b[ \t]*")
            .expect("static regex")
    })
}

/// Compiled filler patterns. An invalid pattern is skipped.
fn compiled_fillers(rules: &Rules) -> Vec<(Regex, String)> {
    rules
        .layer2_filler_patterns
        .iter()
        .filter_map(|p| {
            Regex::new(&format!("(?i){}", p.find))
                .ok()
                .map(|re| (re, p.replace.clone()))
        })
        .collect()
}

/// Applies one level to a span of prose: filler substitution at lite and above,
/// whitespace normalisation at full and above, stopword removal at ultra, then
/// blank-line collapsing.
#[must_use]
pub fn apply_level(text: &str, level: &str, rules: &Rules) -> String {
    let mut piece = text.to_owned();
    if matches!(level, "lite" | "full" | "ultra") {
        for (re, replace) in compiled_fillers(rules) {
            piece = re.replace_all(&piece, replace.as_str()).into_owned();
        }
    }
    if matches!(level, "full" | "ultra") {
        piece = multi_space().replace_all(&piece, " ").into_owned();
        piece = piece
            .split('\n')
            .map(|line| line.trim_start_matches([' ', '\t']))
            .collect::<Vec<_>>()
            .join("\n");
    }
    if level == "ultra" {
        piece = stopwords().replace_all(&piece, "").into_owned();
    }
    blank_run().replace_all(&piece, "\n\n").into_owned()
}

/// Lines of `text` with the trailing newline dropped, so the file's final newline can be rebuilt.
pub(crate) fn split_keep_trailing(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

pub(crate) fn is_fence_line(line: &str) -> bool {
    line.starts_with("```")
}

/// Compresses every prose span of `text`, leaving fenced code blocks, fence lines included, untouched.
#[must_use]
pub fn compress_prose(text: &str, level: &str, rules: &Rules) -> String {
    let mut out = String::new();
    let mut buf = String::new();
    let mut in_code = false;
    for line in split_keep_trailing(text) {
        if is_fence_line(line) {
            if in_code {
                buf.push_str(line);
                buf.push('\n');
                out.push_str(&buf);
                buf.clear();
                in_code = false;
            } else {
                out.push_str(&apply_level(&buf, level, rules));
                buf.clear();
                buf.push_str(line);
                buf.push('\n');
                in_code = true;
            }
            continue;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    if in_code {
        out.push_str(&buf);
    } else {
        out.push_str(&apply_level(&buf, level, rules));
    }
    out
}

/// Result of compressing one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResult {
    pub relative: String,
    pub level: String,
    pub before: usize,
    pub after: usize,
}

/// Compresses one file at `requested` level (or the level its graph degree picks). Unless
/// `dry_run`, the original is kept as `<file>.bak` and the file is rewritten.
///
/// # Errors
///
/// Read, backup, write, or stats failures.
pub fn compress_file(
    rules: &Rules,
    repo_root: &Path,
    graph_path: &Path,
    stats_path: Option<&Path>,
    file: &Path,
    requested: &str,
    dry_run: bool,
) -> io::Result<FileResult> {
    let original = fs::read_to_string(file)?;
    let relative = relative_to(repo_root, file);
    let level = resolve_level(rules, graph_path, &relative, requested);
    let compressed = compress_prose(&original, &level, rules);
    let before = stats::estimate_tokens(&original);
    let after = stats::estimate_tokens(&compressed);

    if !dry_run {
        fs::write(backup_path(file), &original)?;
        fs::write(file, &compressed)?;
        if let Some(stats_path) = stats_path {
            stats::add_record(stats_path, "layer2", &relative, before, after)?;
        }
    }
    Ok(FileResult {
        relative,
        level,
        before,
        after,
    })
}

/// `<file>.bak`, next to the original.
#[must_use]
pub fn backup_path(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(".bak");
    PathBuf::from(name)
}

/// The path relative to the repository, or the path itself when it lies outside.
#[must_use]
pub fn relative_to(repo_root: &Path, file: &Path) -> String {
    match file.strip_prefix(repo_root) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => file.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenced_code_is_left_exactly_as_written() {
        let rules = Rules::default();
        let text =
            "It is important to note that this works.\n```\nIt is important to note that\n```\n";
        let out = compress_prose(text, "lite", &rules);
        assert!(out.contains("It is important to note that\n```"), "{out}");
        assert!(
            !out.starts_with("It is important"),
            "prose must be compressed: {out}"
        );
    }

    #[test]
    fn ultra_removes_stopwords_and_full_normalises_spaces() {
        let rules = Rules::default();
        assert_eq!(
            apply_level("a  very   big  dog", "full", &rules),
            "a very big dog"
        );
        assert_eq!(apply_level("a very big dog", "ultra", &rules), "a big dog");
    }
}
