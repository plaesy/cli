//! Checks that the relative paths a task names exist.
//!
//! Extraction is deterministic: no model and no network.
//!
//! - A backtick token is a path when it is relative, has no whitespace or
//!   placeholder characters, and contains `/`. Bare names such as `tasks.rs`
//!   are prose and are ignored.
//! - A markdown link target is a path when it is relative. Schemes, absolute
//!   paths, and bare anchors are ignored. `#anchor` and `?query` are stripped.
//! - A markdown link resolves against the folder of the file containing it. A
//!   backtick path resolves against the project root.
//! - In `plan.md`, a path followed by `(NEW)` is a file the plan creates, so it
//!   is not checked.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{find_task, project_root_of, TasksError, PLAN_FILE, TASK_FILE};

const PLACEHOLDER_CHARS: &[char] = &[
    ' ', '\t', '@', '*', '?', '<', '>', '{', '}', '|', '$', '"', '\'', ',', ';', '=',
];

/// One relative reference found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub line: usize,
    pub path: String,
    /// True for markdown link targets, which resolve against the file's folder.
    pub is_link: bool,
}

/// Verdict for one checked file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileReport {
    /// File is not present. Reported as skipped, never as passed.
    Skipped,
    Checked {
        total: usize,
        missing: Vec<Reference>,
    },
}

/// Checks `spec.md` and `plan.md` of `task`, writes a report to `out`, and
/// returns the per-file reports.
///
/// # Errors
///
/// [`TasksError::NotFound`] when the task does not exist,
/// [`TasksError::MissingReferences`] when any reference is missing, and I/O
/// failures for files that exist but cannot be read.
pub fn check_task(
    tasks_dir: &Path,
    task: &str,
    out: &mut impl Write,
) -> Result<Vec<(String, FileReport)>, TasksError> {
    let (_, task_dir) =
        find_task(tasks_dir, task).ok_or_else(|| TasksError::NotFound(task.to_owned()))?;
    let root = project_root_of(tasks_dir);

    let mut reports = Vec::new();
    let mut missing_total = 0;
    let mut skipped = 0;
    for name in [TASK_FILE, PLAN_FILE] {
        let path = task_dir.join(name);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                writeln!(out, "{name}: skipped (not present)").map_err(io_err)?;
                skipped += 1;
                reports.push((name.to_owned(), FileReport::Skipped));
                continue;
            }
            Err(e) => return Err(TasksError::io(format!("reading {}", path.display()), e)),
        };
        let refs = extract_references(&text, name == PLAN_FILE);
        let total = refs.len();
        let missing: Vec<Reference> = refs
            .into_iter()
            .filter(|r| {
                let base = if r.is_link {
                    task_dir.as_path()
                } else {
                    root.as_path()
                };
                !base.join(&r.path).exists()
            })
            .collect();
        writeln!(
            out,
            "{name}: checked {total} reference(s), {} missing",
            missing.len()
        )
        .map_err(io_err)?;
        for r in &missing {
            writeln!(out, "  {name}:{}: missing {}", r.line, r.path).map_err(io_err)?;
        }
        missing_total += missing.len();
        reports.push((name.to_owned(), FileReport::Checked { total, missing }));
    }

    if missing_total > 0 {
        writeln!(out, "[ERROR] {missing_total} missing reference(s)").map_err(io_err)?;
        return Err(TasksError::MissingReferences(missing_total));
    }
    if skipped > 0 {
        writeln!(
            out,
            "[!] incomplete: {skipped} file(s) skipped, nothing was checked for them"
        )
        .map_err(io_err)?;
    } else {
        writeln!(out, "[OK] every reference exists").map_err(io_err)?;
    }
    Ok(reports)
}

fn io_err(source: io::Error) -> TasksError {
    TasksError::io("writing report", source)
}

/// Extracts relative paths and links in line order. When `skip_new` is true, a
/// path followed by `(NEW)` is left out.
#[must_use]
pub fn extract_references(content: &str, skip_new: bool) -> Vec<Reference> {
    let mut refs = Vec::new();
    for (idx, line) in content.split('\n').enumerate() {
        let line_no = idx + 1;
        for caps in backtick_token().captures_iter(line) {
            let whole = caps.get(0).expect("group 0 always exists");
            let token = &caps[1];
            if !is_repo_path(token) || (skip_new && is_marked_new(&line[whole.end()..])) {
                continue;
            }
            refs.push(Reference {
                line: line_no,
                path: token.to_owned(),
                is_link: false,
            });
        }
        for caps in markdown_link().captures_iter(line) {
            let whole = caps.get(0).expect("group 0 always exists");
            let Some(target) = link_target(&caps[1]) else {
                continue;
            };
            if skip_new && is_marked_new(&line[whole.end()..]) {
                continue;
            }
            refs.push(Reference {
                line: line_no,
                path: target,
                is_link: true,
            });
        }
    }
    refs
}

/// Reports whether a backtick token is a relative file path.
fn is_repo_path(token: &str) -> bool {
    !token.is_empty()
        && !token.contains(PLACEHOLDER_CHARS)
        && !is_absolute_or_home(token)
        && !has_scheme(token)
        && token.contains('/')
}

/// Returns the relative file part of a markdown link target.
fn link_target(raw: &str) -> Option<String> {
    let raw = raw.split(['#', '?']).next().unwrap_or("");
    if raw.is_empty() || has_scheme(raw) || is_absolute_or_home(raw) {
        return None;
    }
    Some(raw.to_owned())
}

fn is_absolute_or_home(path: &str) -> bool {
    path.starts_with(['/', '\\', '~', '#']) || Path::new(path).is_absolute()
}

fn has_scheme(s: &str) -> bool {
    scheme_re().is_match(s)
}

fn is_marked_new(rest: &str) -> bool {
    rest.trim_start_matches([' ', '\t']).starts_with("(NEW)")
}

fn backtick_token() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new("`([^`]+)`").expect("static regex"))
}

fn markdown_link() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[[^\]]*\]\(([^)\s]+)\)").expect("static regex"))
}

fn scheme_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.\-]*:").expect("static regex"))
}

/// Human-readable summary of a [`FileReport`], used by callers that print one line per file.
#[must_use]
pub fn summarize(name: &str, report: &FileReport) -> String {
    let mut s = String::new();
    match report {
        FileReport::Skipped => {
            let _ = write!(s, "{name}: skipped (not present)");
        }
        FileReport::Checked { total, missing } => {
            let _ = write!(
                s,
                "{name}: checked {total} reference(s), {} missing",
                missing.len()
            );
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tasks::{add_task, tasks_dir};

    #[test]
    fn backtick_paths_need_a_slash_and_no_placeholders() {
        let refs = extract_references(
            "see `src/lib.rs` and `tasks.rs` and `plaesy tasks check` and `a/<b>`",
            false,
        );
        let paths: Vec<_> = refs.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, ["src/lib.rs"]);
    }

    #[test]
    fn links_skip_schemes_absolute_paths_and_anchors_strip() {
        let refs = extract_references(
            "[a](https://x.y) [b](/abs/p) [c](#top) [d](docs/x.md#sec) [e](../up.md?q=1)",
            false,
        );
        let paths: Vec<_> = refs.iter().map(|r| (r.path.as_str(), r.is_link)).collect();
        assert_eq!(paths, [("docs/x.md", true), ("../up.md", true)]);
    }

    #[test]
    fn new_marker_excludes_planned_files_in_plan_only() {
        let text = "`src/new.rs` (NEW) and `src/old.rs`";
        assert_eq!(extract_references(text, true).len(), 1);
        assert_eq!(extract_references(text, false).len(), 2);
    }

    #[test]
    fn check_reports_missing_and_passes_when_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".plaesy")).unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/real.rs"), "").unwrap();
        let tasks = tasks_dir(dir.path());
        let name = add_task(&tasks, "s", "low", "chk", "Chk").unwrap();
        let spec = tasks.join("backlog").join(&name).join(TASK_FILE);
        std::fs::write(
            &spec,
            "---\nspec: s\n---\n`src/real.rs` and `src/gone.rs`\n",
        )
        .unwrap();

        let mut out = Vec::new();
        let err = check_task(&tasks, &name, &mut out).unwrap_err();
        assert!(matches!(err, TasksError::MissingReferences(1)));
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("spec.md:4: missing src/gone.rs"));

        std::fs::write(&spec, "`src/real.rs`\n").unwrap();
        let mut ok_out = Vec::new();
        check_task(&tasks, &name, &mut ok_out).unwrap();
        assert!(String::from_utf8(ok_out)
            .unwrap()
            .contains("[!] incomplete"));
    }
}
