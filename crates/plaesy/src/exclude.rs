//! Paths that repo-wide scans leave out.
//!
//! Every walker asks this module rather than keeping its own list, so a folder
//! excluded in one output is excluded in all of them.
//! Port of `scripts/internal/exclude` (Go).

use std::fs;
use std::path::Path;

use regex::Regex;

/// Directory base names that hold dependencies, build output, or tool caches.
/// Dot-directories are excluded by [`dir_name`]'s prefix rule instead.
const DIR_NAMES: &[&str] = &[
    "node_modules",
    "vendor",
    "dist",
    "build",
    "target",
    "__pycache__",
    "venv",
    "Pods",
    "DerivedData",
    "coverage",
];

/// Generated copy of the templates, under the Go tree. Its sources are walked in place.
const GENERATED_ASSETS_DIR: &str = "scripts/internal/assets/data";

/// Reports whether a directory with this base name is excluded.
#[must_use]
pub fn dir_name(name: &str) -> bool {
    !name.is_empty() && (name.starts_with('.') || DIR_NAMES.contains(&name))
}

/// Reports whether a slash-separated path relative to the root is excluded by location.
#[must_use]
pub fn rel_path(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    lower.ends_with("/flutter/ephemeral")
        || lower == "flutter/ephemeral"
        || lower == GENERATED_ASSETS_DIR
        || lower.starts_with(&format!("{GENERATED_ASSETS_DIR}/"))
}

/// Root `.gitignore` rules. The empty value matches nothing.
#[derive(Debug, Default)]
pub struct Ignore {
    rules: Vec<Rule>,
}

#[derive(Debug)]
struct Rule {
    pattern: Regex,
    negate: bool,
    dir_only: bool,
}

impl Ignore {
    /// Reads `root/.gitignore`. A missing or unreadable file yields no rules.
    /// Nested `.gitignore` files are not read.
    #[must_use]
    pub fn load(root: &Path) -> Self {
        let Ok(text) = fs::read_to_string(root.join(".gitignore")) else {
            return Self::default();
        };
        Self {
            rules: text.lines().filter_map(parse_rule).collect(),
        }
    }

    /// Reports whether `rel` is ignored. The last matching rule decides, so a
    /// later `!` pattern re-includes a path.
    #[must_use]
    pub fn matches(&self, rel: &str, is_dir: bool) -> bool {
        self.rules
            .iter()
            .filter(|r| !r.dir_only || is_dir)
            .fold(false, |ignored, r| {
                if r.pattern.is_match(rel) {
                    !r.negate
                } else {
                    ignored
                }
            })
    }

    /// Reports whether a directory must not be entered.
    #[must_use]
    pub fn skip_dir(&self, rel: &str, name: &str) -> bool {
        dir_name(name) || rel_path(rel) || self.matches(rel, true)
    }

    /// Reports whether a file must be left out of a scan.
    #[must_use]
    pub fn skip_file(&self, rel: &str) -> bool {
        self.matches(rel, false)
    }

    /// Reports whether a document index must not enter a directory.
    ///
    /// Same as [`Ignore::skip_dir`], except the memory and decision folders under
    /// `.plaesy` are entered, and `.plaesy` itself only to reach them.
    #[must_use]
    pub fn skip_index_dir(&self, rel: &str, name: &str) -> bool {
        if rel == ".plaesy" || in_indexed_plaesy_dir(rel) {
            return false;
        }
        if rel.starts_with(".plaesy/") {
            return true;
        }
        self.skip_dir(rel, name)
    }

    /// Reports whether a document index must leave out a file. Under `.plaesy`,
    /// only memory and decision records are indexed.
    #[must_use]
    pub fn skip_index_file(&self, rel: &str) -> bool {
        if rel.starts_with(".plaesy/") && !in_indexed_plaesy_dir(rel) {
            return true;
        }
        self.skip_file(rel)
    }
}

const INDEXED_PLAESY_DIRS: &[&str] = &[".plaesy/memory", ".plaesy/decisions"];

fn in_indexed_plaesy_dir(rel: &str) -> bool {
    INDEXED_PLAESY_DIRS
        .iter()
        .any(|dir| rel == *dir || rel.starts_with(&format!("{dir}/")))
}

/// Turns one `.gitignore` line into a rule. Blank lines and comments yield none.
fn parse_rule(line: &str) -> Option<Rule> {
    let mut line = line.trim_end_matches([' ', '\t', '\r']);
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut negate = false;
    if let Some(rest) = line.strip_prefix('!') {
        negate = true;
        line = rest;
    } else if let Some(rest) = line.strip_prefix('\\') {
        line = rest;
    }
    let mut dir_only = false;
    if let Some(rest) = line.strip_suffix('/') {
        dir_only = true;
        line = rest;
    }
    let anchored = line.contains('/');
    let line = line.trim_start_matches('/');
    if line.is_empty() {
        return None;
    }
    let glob = glob_to_regex(line);
    let source = if anchored {
        format!("^{glob}$")
    } else {
        format!("(?:^|/){glob}$")
    };
    Regex::new(&source).ok().map(|pattern| Rule {
        pattern,
        negate,
        dir_only,
    })
}

/// Converts a gitignore glob into an unanchored regex fragment.
fn glob_to_regex(glob: &str) -> String {
    let mut out = String::new();
    let mut rest = glob;
    while let Some(c) = rest.chars().next() {
        if let Some(tail) = rest.strip_prefix("**/") {
            out.push_str("(?:.*/)?");
            rest = tail;
        } else if rest == "/**" {
            out.push_str("/.*");
            rest = "";
        } else if let Some(tail) = rest.strip_prefix("**") {
            out.push_str(".*");
            rest = tail;
        } else if c == '*' {
            out.push_str("[^/]*");
            rest = &rest[1..];
        } else if c == '?' {
            out.push_str("[^/]");
            rest = &rest[1..];
        } else if c == '[' {
            match rest[1..].find(']') {
                Some(end) => {
                    let class = &rest[1..=end];
                    let class = class
                        .strip_prefix('!')
                        .map_or_else(|| class.to_owned(), |negated| format!("^{negated}"));
                    out.push('[');
                    out.push_str(&class);
                    out.push(']');
                    rest = &rest[end + 2..];
                }
                None => {
                    out.push_str(&regex::escape("["));
                    rest = &rest[1..];
                }
            }
        } else if c == '\\' && rest.len() > 1 {
            let next = rest[1..].chars().next().expect("length checked");
            out.push_str(&regex::escape(&next.to_string()));
            rest = &rest[1 + next.len_utf8()..];
        } else {
            out.push_str(&regex::escape(&c.to_string()));
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_and_known_dependency_directories_are_excluded() {
        assert!(dir_name(".git"));
        assert!(dir_name(".plaesy"));
        assert!(dir_name("node_modules"));
        assert!(!dir_name("src"));
        assert!(!dir_name(""));
    }

    #[test]
    fn flutter_ephemeral_and_generated_assets_are_excluded_by_location() {
        assert!(rel_path("ios/Flutter/ephemeral"));
        assert!(rel_path("linux/flutter/ephemeral"));
        assert!(rel_path("scripts/internal/assets/data/skills/x.md"));
        assert!(!rel_path("scripts/internal/assets/gen"));
    }

    #[test]
    fn gitignore_rules_apply_last_match_wins() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".gitignore"),
            "# comment\n*.log\n!keep.log\n/out/\nsecret/**\n",
        )
        .unwrap();
        let ig = Ignore::load(dir.path());
        assert!(ig.skip_file("debug.log"));
        assert!(ig.skip_file("nested/debug.log"));
        assert!(!ig.skip_file("keep.log"));
        assert!(ig.skip_dir("out", "out"));
        assert!(!ig.skip_file("out"), "dir-only rule must not match a file");
        assert!(
            !ig.skip_file("src/out"),
            "anchored rule only matches at the root"
        );
        assert!(ig.skip_file("secret/key.txt"));
    }

    #[test]
    fn missing_gitignore_ignores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let ig = Ignore::load(dir.path());
        assert!(!ig.skip_file("anything.log"));
    }

    #[test]
    fn index_walk_enters_only_memory_and_decisions_under_plaesy() {
        let ig = Ignore::default();
        assert!(!ig.skip_index_dir(".plaesy", ".plaesy"));
        assert!(!ig.skip_index_dir(".plaesy/memory", "memory"));
        assert!(ig.skip_index_dir(".plaesy/analysis", "analysis"));
        assert!(!ig.skip_index_file(".plaesy/decisions/x.md"));
        assert!(ig.skip_index_file(".plaesy/context.md"));
    }

    #[test]
    fn bracket_classes_and_escapes_translate() {
        let re = Regex::new(&format!("^{}$", glob_to_regex("file[!0-9].txt"))).unwrap();
        assert!(re.is_match("filex.txt"));
        assert!(!re.is_match("file5.txt"));
        let esc = Regex::new(&format!("^{}$", glob_to_regex(r"a\*b"))).unwrap();
        assert!(esc.is_match("a*b"));
        assert!(!esc.is_match("axb"));
    }
}
