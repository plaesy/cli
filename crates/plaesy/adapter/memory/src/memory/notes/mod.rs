//! Writing notes on behalf of an agent (`memory_remember` over MCP).
//!
//! The memory's only writes to the notes folder are new Markdown files. A file that already
//! exists is never overwritten, the folder must stay inside the notes root, and the new note is
//! indexed at once so it can be cited straight away.

use crate::memory::error::{MemoryError, Result};
use crate::memory::search::{SearchEngine, TIERS};
use chrono::{NaiveDate, Utc};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::Duration;

mod engine;
mod lock;
mod session;
mod template;

pub use session::{SessionLink, SessionSummary, MAX_SESSION_LINKS, SESSION_FOLDER, SESSION_TIER};
// Used by the test modules via `super::*`.
#[cfg(test)]
use engine::strip_last_verified;
pub(crate) use lock::*;
pub(crate) use template::*;

/// Confidence values a note may declare.
pub const CONFIDENCE: [&str; 3] = ["high", "medium", "low"];

/// Longest title, in characters.
pub const MAX_TITLE_CHARS: usize = 200;

/// Largest body an agent may write in one note, in bytes.
pub const MAX_BODY_BYTES: usize = 256 * 1024;

/// A note an agent wants to add.
#[derive(Clone, Debug)]
pub struct NewNote {
    /// Folder relative to the notes root. Empty means `inbox`.
    pub folder: String,
    /// Title; also the basis of the file name.
    pub title: String,
    /// Markdown body.
    pub body: String,
    /// One of [`TIERS`].
    pub tier: String,
    /// One of [`CONFIDENCE`].
    pub confidence: String,
    /// Optional projects/decisions/contexts graph tagging (see `crate::memory::graph`).
    pub graph: Option<GraphFields>,
}

/// Graph frontmatter for a new note, mirroring `crate::memory::graph`'s contract
/// (`type`/`id`/`status`/`project`/`scope`/`supersedes`). Validated against
/// [`crate::memory::graph::NOTE_TYPES`] before the note is written.
#[derive(Clone, Debug, Default)]
pub struct GraphFields {
    /// `type`: project, decision, context, knowledge, capture, preference.
    pub note_type: String,
    /// `id`: stable identifier, referenced by `project` / `scope` / `supersedes`.
    pub id: String,
    /// `status`, if given (else the graph module's per-type default applies).
    pub status: Option<String>,
    /// `project`: decision/context's main project.
    pub project: Option<String>,
    /// `scope`: other projects this note is also relevant to.
    pub scope: Vec<String>,
    /// `supersedes`: ids of decisions this one replaces.
    pub supersedes: Vec<String>,
}

/// File-name slug of a title: lower-case ASCII words joined by `-`, at most 80 characters.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed: String = out.trim_end_matches('-').chars().take(80).collect();
    if trimmed.is_empty() {
        // Titles with no ASCII letters (for example Japanese or Arabic) would all become the same
        // name; a hash of the whole title keeps them apart. FNV-1a, so the name is stable.
        let hash = title.bytes().fold(0x811c_9dc5_u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        });
        format!("note-{hash:08x}")
    } else {
        trimmed
    }
}

/// Validate a folder: relative, no `..`, no drive or root prefix. Empty means `inbox`.
pub fn safe_folder(folder: &str) -> Result<PathBuf> {
    let folder = folder.trim().trim_matches(|c| c == '/' || c == '\\');
    if folder.is_empty() {
        return Ok(PathBuf::from("inbox"));
    }
    let path = PathBuf::from(folder);
    for part in path.components() {
        match part {
            Component::Normal(_) => {}
            _ => {
                return Err(MemoryError::ConfigError(format!(
                    "folder must be relative to the notes root without '..': {folder}"
                )));
            }
        }
    }
    Ok(path)
}

fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// The vault's inbox when no folder is given: an existing top-level folder whose name is `inbox`
/// once any numeric prefix such as `00. ` is ignored (so `00. Inbox` is found), else `inbox`.
pub fn default_inbox(root: &std::path::Path) -> PathBuf {
    let found = std::fs::read_dir(root).ok().and_then(|entries| {
        entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| {
                        n.trim_start_matches(|c: char| {
                            c.is_ascii_digit() || matches!(c, '.' | ' ' | '-' | '_')
                        })
                        .eq_ignore_ascii_case("inbox")
                    })
                    .unwrap_or(false)
            })
    });
    match found.and_then(|p| p.file_name().map(PathBuf::from)) {
        Some(name) => name,
        None => PathBuf::from("inbox"),
    }
}

/// Front matter and body of a new note.
pub fn render(note: &NewNote, today: NaiveDate) -> String {
    // serde_json::to_string on a &str produces a double-quoted, properly escaped string that
    // is also valid YAML (YAML's double-quoted scalar syntax is a superset of JSON's) — this is
    // the only safe way to embed agent-supplied text as a YAML value, since any of these fields
    // may contain ':', '"', '#', or other characters that break bare/naively-quoted YAML.
    //
    // JSON itself is not quite enough: JSON permits raw 0x7F (DEL) and the C1 control range
    // (0x80-0x9F) unescaped inside a string, but YAML's double-quoted scalar grammar forbids
    // raw control characters outside its printable-character set. A title/id/project containing
    // such a byte (e.g. a stray DEL from clipboard noise) would otherwise produce frontmatter
    // that fails to parse as YAML, silently dropping the note from the graph. So every control
    // character is re-escaped as `\xHH` after JSON encoding, which is also valid YAML syntax.
    fn yaml_str(s: &str) -> String {
        let json = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
        let mut out = String::with_capacity(json.len());
        for c in json.chars() {
            let code = c as u32;
            let is_noncharacter = (0xFDD0..=0xFDEF).contains(&code) || (code & 0xFFFF) >= 0xFFFE;
            if is_noncharacter {
                // YAML forbids Unicode noncharacters even in escaped form. Replace them with
                // the standard replacement character so arbitrary titles remain parseable.
                out.push('\u{FFFD}');
            } else if c.is_control() && c != '\u{0}' {
                // '\u{0}' never reaches here since JSON already escaped it as \u0000.
                out.push_str(&format!("\\x{:02X}", code));
            } else {
                out.push(c);
            }
        }
        out
    }
    fn yaml_list(items: &[String]) -> String {
        let rendered: Vec<String> = items.iter().map(|s| yaml_str(s)).collect();
        format!("[{}]", rendered.join(", "))
    }
    let title = yaml_str(&note.title);
    let mut graph_lines = String::new();
    if let Some(g) = &note.graph {
        graph_lines.push_str(&format!("type: {}\n", yaml_str(&g.note_type)));
        graph_lines.push_str(&format!("id: {}\n", yaml_str(&g.id)));
        if let Some(status) = &g.status {
            graph_lines.push_str(&format!("status: {}\n", yaml_str(status)));
        }
        if let Some(project) = &g.project {
            graph_lines.push_str(&format!("project: {}\n", yaml_str(project)));
        }
        if !g.scope.is_empty() {
            graph_lines.push_str(&format!("scope: {}\n", yaml_list(&g.scope)));
        }
        if !g.supersedes.is_empty() {
            graph_lines.push_str(&format!("supersedes: {}\n", yaml_list(&g.supersedes)));
        }
    }
    format!(
        "---\ntitle: {title}\ntier: {tier}\nconfidence: {conf}\nlast_verified: {today}\nsource: agent\n{graph_lines}---\n\n# {heading}\n\n{body}\n",
        tier = note.tier,
        conf = note.confidence,
        heading = note.title,
        body = note.body.trim_end(),
    )
}

#[cfg(test)]
#[path = "notes_test.rs"]
mod notes_test;

/// Property-based adversarial fuzzing for the pure, boundary-facing helpers in this module.
/// These take arbitrary agent/user-controlled strings and must never panic, never escape the
/// notes root, and must always round-trip through valid YAML.
#[cfg(test)]
#[path = "notes_fuzz_test.rs"]
mod notes_fuzz_test;
