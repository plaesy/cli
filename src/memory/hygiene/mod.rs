//! Memory hygiene: find notes whose trust metadata is missing, stale or contradicted by the
//! current state of the project.
//!
//! The failure this guards against is *stale but confident* memory: a note that was true when
//! written ("the config lives at `x/y.toml`") keeps ranking high after the world changed. The
//! checks are cheap and read-only (the report never edits files):
//!
//! | kind | meaning | label |
//! |------|---------|-------|
//! | `missing_source` | a core/semantic/procedural note has no non-empty provenance source | FACT |
//! | `missing_last_verified` / `missing_confidence` | a core/semantic/procedural note declares no verification date or trust | FACT |
//! | `stale` | `last_verified` is older than the staleness window | FACT |
//! | `bad_tier` | frontmatter `tier` is not one of the known tiers | FACT |
//! | `broken_link` | a `[[wikilink]]` resolves to no indexed document | FACT |
//! | `broken_markdown_link` / `broken_link_anchor` | a local Markdown destination or heading anchor is missing | FACT / INFERRED |
//! | `machine_specific_link` | a local Markdown link depends on one device's filesystem | FACT |
//! | `superseded_without_replacement` | `status: superseded` but no `superseded_by` | FACT |
//! | `dead_path_reference` | a backticked relative file path does not exist in the note's folder hierarchy, its project, the vault, or a mapped local source root | INFERRED |
//! | `possible_secret` | a credential assignment or private-key header appears in note text; value is never returned | INFERRED |

//! Secret findings report only kind and line number. They never include the matched value.

//! Set [`crate::memory::hygiene::HygieneOptions::scan_secrets`] to `false` only for compatibility
//! with a caller that explicitly does not want this check.

//! The check intentionally favours recall over certainty; findings require human review.

//! It ignores fenced code blocks to avoid flagging examples and source snippets.

//! It is not a replacement for a dedicated secret scanner.
//!
//! `dead_path_reference` is a heuristic (the path may legitimately refer to another repository
//! that is not checked out or mapped locally), hence `INFERRED`. `possible_secret` is also `INFERRED`: it looks for credential assignments and
//! private-key headers, but cannot prove that a usable credential is present. Other findings are
//! read from indexed content or metadata.

use crate::memory::error::Result;
use crate::memory::search::{age_days, build_meta, normalize_tier, parse_date, SearchEngine};
use crate::memory::storage::LinkRef;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod paths;
mod scan;
mod secrets;
pub use paths::*;
pub use secrets::*;

mod markdown_links;

/// Check local Markdown links without opening or updating the memory index.
///
/// This is intentionally read-only so it can be used as a fast vault/project check.
pub fn validate_markdown_links(
    root: &std::path::Path,
) -> crate::memory::error::Result<Vec<Finding>> {
    validate_markdown_links_in_scope(root, root)
}

/// Validate links from Markdown files under `scope` against the full `vault_root` boundary.
///
/// The scope must be a directory inside the vault. Link targets may cross the scope boundary,
/// but cannot leave the vault. This function reads files only and never modifies the index.
pub fn validate_markdown_links_in_scope(
    vault_root: &std::path::Path,
    scope: &std::path::Path,
) -> crate::memory::error::Result<Vec<Finding>> {
    use crate::memory::indexer::Extractor;
    use crate::memory::storage::StoredDocument;

    let canonicalize = |path: &std::path::Path| {
        path.canonicalize()
            .map_err(|source| crate::memory::error::MemoryError::IoError {
                path: path.display().to_string(),
                source,
            })
    };
    let root = canonicalize(vault_root)?;
    let scope = canonicalize(scope)?;
    if !root.is_dir() || !scope.is_dir() {
        return Err(crate::memory::error::MemoryError::ConfigError(
            "link validation vault and scope must be directories".into(),
        ));
    }
    if !scope.starts_with(&root) {
        return Err(crate::memory::error::MemoryError::ConfigError(format!(
            "link validation scope is outside vault root: {}",
            scope.display()
        )));
    }
    let extractor = Extractor::new();
    let mut docs = Vec::new();
    for entry in walkdir::WalkDir::new(&scope)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            entry.depth() == 0
                || !(entry.file_type().is_dir()
                    && matches!(
                        entry.file_name().to_string_lossy().as_ref(),
                        ".git" | "node_modules" | "target"
                    ))
        })
    {
        let entry = entry
            .map_err(|error| crate::memory::error::MemoryError::ConfigError(error.to_string()))?;
        let path = entry.path();
        let is_markdown = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
            });
        if !entry.file_type().is_file() || !is_markdown {
            continue;
        }
        let document = match extractor.extract_file(path) {
            Ok(document) => document,
            Err(crate::memory::error::MemoryError::ExtractionError { reason, .. })
                if reason == "file is empty" =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        docs.push(StoredDocument {
            path: path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/"),
            metadata: document.metadata,
            body: document.body,
            body_start_line: document.body_start_line,
        });
    }
    docs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(markdown_links::check(&root, &docs))
}

/// Options for [`SearchEngine::hygiene`].
#[derive(Clone, Debug)]
pub struct HygieneOptions {
    /// Directory the indexed paths are relative to. Enables filesystem-backed checks for local
    /// Markdown links and `dead_path_reference`, which tries each ancestor directory up to the
    /// project root for `projects/<slug>/...` notes. `project_roots` adds an optional, local source
    /// repository roots for a matching project slug; a leading path component matching a checkout
    /// directory name is also accepted.
    pub root: Option<PathBuf>,

    /// Optional vault-relative directory prefix restricting findings and checked documents.
    pub scope: Option<PathBuf>,

    /// Additional local source repository roots keyed by the vault project slug. These paths
    /// remain device-local and are used only to resolve backticked repository-relative paths.
    pub project_roots: BTreeMap<String, Vec<PathBuf>>,

    /// Overrides the engine's `stale_after_days`.
    pub stale_after_days: Option<u32>,

    /// Clock override (tests).
    pub now: Option<DateTime<Utc>>,

    /// Scan note text for likely credentials without returning their values.
    pub scan_secrets: bool,
}

impl Default for HygieneOptions {
    fn default() -> Self {
        Self {
            root: None,
            scope: None,
            project_roots: BTreeMap::new(),
            stale_after_days: None,
            now: None,
            scan_secrets: true,
        }
    }
}

/// One problem found in the memory.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    /// Finding kind (see module docs).
    pub kind: String,

    /// Document the finding is about.
    pub path: String,

    /// 1-based line, when the finding points at one.
    pub line: Option<usize>,

    /// Human-readable explanation.
    pub detail: String,

    /// `FACT` (read from the index) or `INFERRED` (heuristic).
    pub label: String,
}

/// Result of a hygiene run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HygieneReport {
    /// Number of documents checked.
    pub checked: usize,

    /// Findings, ordered by path then line.
    pub findings: Vec<Finding>,

    /// Count per finding kind.
    pub summary: BTreeMap<String, usize>,
}

fn finding(kind: &str, path: &str, line: Option<usize>, detail: String, label: &str) -> Finding {
    Finding {
        kind: kind.to_string(),
        path: path.to_string(),
        line,
        detail,
        label: label.to_string(),
    }
}

#[cfg(test)]
#[path = "hygiene_test.rs"]
mod hygiene_test;
