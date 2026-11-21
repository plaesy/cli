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
use std::io::{self, Write};
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{
    find_task, plan_doc, project_root_of, spec_doc, topic_of, TasksError, PLAN_FILE, SPEC_FILE,
};

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

/// Checks the `spec.md` and `plan.md` of the task's topic, writes a report to `out`,
/// and returns the per-file reports.
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
    let (status, _) =
        find_task(tasks_dir, task).ok_or_else(|| TasksError::NotFound(task.to_owned()))?;
    let root = project_root_of(tasks_dir);
    let topic =
        topic_of(tasks_dir, status, task).ok_or_else(|| TasksError::NoTopic(task.to_owned()))?;
    let docs = [
        (SPEC_FILE, spec_doc(&root, &topic)),
        (PLAN_FILE, plan_doc(&root, &topic)),
    ];

    let mut reports = Vec::new();
    let mut missing_total = 0;
    let mut skipped = 0;
    for (name, path) in docs {
        let doc_dir = path.parent().unwrap_or(&root).to_path_buf();
        let relative = path.strip_prefix(&root).map_err(|error| {
            TasksError::io(
                format!("invalid task document path {}", path.display()),
                io::Error::new(io::ErrorKind::InvalidInput, error),
            )
        })?;
        let root_handle = crate::memory::fs_safety::DirHandle::open_nofollow(&root)
            .map_err(|error| TasksError::io("opening project root safely", error))?;
        let text = match root_handle
            .open_file_read_relative_nofollow(relative)
            .and_then(|mut file| {
                let mut text = String::new();
                std::io::Read::read_to_string(&mut file, &mut text)?;
                Ok(text)
            }) {
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
                    doc_dir.as_path()
                } else {
                    root.as_path()
                };
                let candidate = base.join(&r.path);
                let Ok(root) = root.canonicalize() else {
                    return true;
                };
                let Ok(candidate) = candidate.canonicalize() else {
                    return true;
                };
                !candidate.starts_with(root)
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
#[path = "check_test.rs"]
mod check_test;
