//! Read-only memory curation reports.
//!
//! Curation combines deterministic health checks with review candidates. It never edits, moves,
//! archives, or deletes vault files; those choices remain with the vault owner.

use crate::memory::error::Result;
use crate::memory::gaps::QueryGap;
use crate::memory::hygiene::{HygieneOptions, HygieneReport};
use crate::memory::search::SearchEngine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

const DEFAULT_TOPIC_INDEX_THRESHOLD: usize = 5;

/// A bounded list in a curation report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurationList<T> {
    /// Items included in this report.
    pub items: Vec<T>,
    /// More items exist than the requested report limit.
    pub has_more: bool,
}

/// A knowledge folder that may benefit from an index note.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicIndexCandidate {
    /// Folder relative to the selected vault root.
    pub folder: String,
    /// Number of direct Markdown notes in the folder, excluding its index files.
    pub notes: usize,
}

/// Files in one exact-content duplicate group.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DuplicateGroup {
    /// Bounded paths that have identical source content.
    pub files: CurationList<String>,
}

/// Read-only curation findings for a Markdown vault.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurationReport {
    /// Schema version for machine-readable consumers.
    pub schema_version: u8,
    /// UTC generation time in RFC 3339 format.
    pub generated_at: String,
    /// Number of indexed notes checked by hygiene.
    pub checked: usize,
    /// Bounded deterministic metadata, link, path, and graph findings.
    pub hygiene: HygieneReport,
    /// Total hygiene findings before the report limit was applied.
    pub hygiene_findings_total: usize,
    /// Total confirmed hygiene findings before the report limit was applied.
    pub confirmed_findings: usize,
    /// Total inferred hygiene findings before the report limit was applied.
    pub inferred_findings: usize,
    /// More hygiene findings exist than are included in `hygiene.findings`.
    pub hygiene_has_more: bool,
    /// Searches that repeatedly found no useful evidence.
    pub query_gaps: CurationList<QueryGap>,
    /// Notes with byte-for-byte identical indexed source content.
    pub exact_duplicates: CurationList<DuplicateGroup>,
    /// Markdown notes with no incoming or outgoing wikilinks; review candidates only.
    pub unlinked_notes: CurationList<String>,
    /// Knowledge folders with enough direct notes but no README, index, or MOC note.
    pub missing_topic_indexes: CurationList<TopicIndexCandidate>,
    /// Number of direct topic notes required before suggesting an index note.
    pub topic_index_threshold: usize,
    /// This command does not modify source notes.
    pub read_only: bool,
}

impl SearchEngine {
    /// Build a bounded, read-only report from indexed memory and vault structure.
    pub async fn curate(
        &self,
        hygiene_options: &HygieneOptions,
        limit: usize,
    ) -> Result<CurationReport> {
        let limit = limit.clamp(1, 100);
        let mut hygiene = self.hygiene(hygiene_options).await?;
        let hygiene_findings_total = hygiene.findings.len();
        let confirmed_findings = hygiene
            .findings
            .iter()
            .filter(|finding| finding.label == "FACT")
            .count();
        let inferred_findings = hygiene
            .findings
            .iter()
            .filter(|finding| finding.label == "INFERRED")
            .count();
        let hygiene_has_more = hygiene_findings_total > limit;
        hygiene.findings.truncate(limit);
        let db = self.database();
        let query_gaps = db.open_query_gaps(limit + 1, Utc::now())?;
        let query_gaps = bounded(query_gaps, limit);
        let exact_duplicates = bounded(
            db.exact_duplicate_documents()
                .await?
                .into_iter()
                .map(|paths| DuplicateGroup {
                    files: bounded(paths, limit),
                })
                .collect(),
            limit,
        );

        let document_paths: Vec<String> = db
            .document_paths()
            .await?
            .into_iter()
            .map(|(_, path)| path)
            .collect();
        let links = db.all_links().await?;
        let mut linked = HashSet::new();
        for link in links {
            linked.insert(link.source_path);
            if let Some(target) = link.resolved_path {
                linked.insert(target);
            }
        }
        let unlinked_notes = bounded(
            document_paths
                .iter()
                .filter(|path| reviewable_note(path) && !linked.contains(*path))
                .cloned()
                .collect(),
            limit,
        );
        let missing_topic_indexes = bounded(topic_index_candidates(&document_paths), limit);

        Ok(CurationReport {
            schema_version: 1,
            generated_at: Utc::now().to_rfc3339(),
            checked: hygiene.checked,
            hygiene,
            hygiene_findings_total,
            confirmed_findings,
            inferred_findings,
            hygiene_has_more,
            query_gaps,
            exact_duplicates,
            unlinked_notes,
            missing_topic_indexes,
            topic_index_threshold: DEFAULT_TOPIC_INDEX_THRESHOLD,
            read_only: true,
        })
    }
}

fn bounded<T>(mut items: Vec<T>, limit: usize) -> CurationList<T> {
    let has_more = items.len() > limit;
    items.truncate(limit);
    CurationList { items, has_more }
}

fn reviewable_note(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if !lower.ends_with(".md") && !lower.ends_with(".markdown") {
        return false;
    }
    let segments: Vec<&str> = lower.split('/').collect();
    if segments.iter().any(|segment| {
        matches!(
            *segment,
            ".plaesy" | "templates" | "archive" | "archives" | "inbox" | "sandbox"
        )
    }) {
        return false;
    }
    !matches!(
        segments.last().copied(),
        Some("readme.md" | "index.md" | "moc.md" | "knowledges.md" | "decissions.md")
    )
}

fn topic_index_candidates(paths: &[String]) -> Vec<TopicIndexCandidate> {
    #[derive(Default)]
    struct FolderState {
        notes: usize,
        has_index: bool,
    }

    let mut folders: BTreeMap<String, FolderState> = BTreeMap::new();
    for path in paths {
        let normalized = path.replace('\\', "/");
        let lower = normalized.to_ascii_lowercase();
        if !lower.starts_with("knowledges/")
            || (!lower.ends_with(".md") && !lower.ends_with(".markdown"))
        {
            continue;
        }
        let Some((folder, file)) = normalized.rsplit_once('/') else {
            continue;
        };
        let state = folders.entry(folder.to_owned()).or_default();
        if matches!(
            file.to_ascii_lowercase().as_str(),
            "readme.md" | "index.md" | "moc.md"
        ) {
            state.has_index = true;
        } else {
            state.notes += 1;
        }
    }

    folders
        .into_iter()
        .filter_map(|(folder, state)| {
            (state.notes >= DEFAULT_TOPIC_INDEX_THRESHOLD && !state.has_index).then_some(
                TopicIndexCandidate {
                    folder,
                    notes: state.notes,
                },
            )
        })
        .collect()
}
