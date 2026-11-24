//! A structured view over supported graph-note types. Vaults may use additional `type` values
//! for ordinary metadata; those notes remain searchable without becoming graph nodes.
//!
//! This builds on the metadata every note already has (YAML frontmatter stored verbatim as
//! JSON in `documents.metadata`); no new SQL column or migration is needed. A note opts in by
//! declaring:
//!
//! ```yaml
//! ---
//! type: project            # project | decision | context | knowledge | capture | preference
//! id: studio-a             # stable identifier, referenced by `project` / `scope` / `supersedes`
//! status: active           # see STATUSES below for the allowed values per type
//! project: studio-a        # decision/context: the project it mainly belongs to
//! scope: [studio-a, studio-b]   # optional: also relevant to these projects
//! supersedes: [old-decision-id] # decision: ids of decisions this one replaces
//! ---
//! ```
//!
//! Notes without a graph `type` are unaffected: they keep appearing in `memory search`, just not
//! here. Templates and recognized metadata-only types are also excluded. A graph note missing its
//! required `id` is skipped from every listing (never crashes the command) and surfaced instead as
//! a `memory hygiene` finding.

use crate::memory::storage::{Database, StoredDocument};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// Recognised values for the `type` field.
pub const NOTE_TYPES: [&str; 6] = [
    "project",
    "decision",
    "context",
    "knowledge",
    "capture",
    "preference",
];

/// Vault types that carry useful metadata but do not participate in this graph.
///
/// Keep these aligned with the vault's note templates and established report types. This avoids
/// treating valid notes as malformed graph records merely because they have a `type` field.
const NON_GRAPH_NOTE_TYPES: [&str; 7] = [
    "company",
    "idea",
    "memory",
    "notes",
    "proposal",
    "task",
    "validation-report",
];

/// Allowed `status` values for `type: project`. Unknown/missing status defaults to `active`.
pub const PROJECT_STATUSES: [&str; 4] = ["active", "paused", "completed", "archived"];
/// Allowed `status` values for `type: decision`. Unknown/missing status defaults to `active`.
pub const DECISION_STATUSES: [&str; 4] = ["proposed", "active", "superseded", "revoked"];
/// Allowed `status` values for `type: context`. Unknown/missing status defaults to `active`.
pub const CONTEXT_STATUSES: [&str; 2] = ["active", "archived"];

/// Statuses hidden by default unless `include_inactive` or an explicit `--status` is given.
fn inactive_statuses(note_type: &str) -> &'static [&'static str] {
    match note_type {
        "project" => &["completed", "archived"],
        "decision" => &["superseded", "revoked"],
        "context" | "capture" => &["archived"],
        _ => &[],
    }
}

/// One note participating in the project/decision/context graph.
#[derive(Clone, Debug, Serialize)]
pub struct GraphNote {
    /// Path relative to the notes root.
    pub path: String,
    /// Declared `id`. Always `Some` for a note returned by [`load_graph`] (malformed notes with
    /// no id are routed to `malformed` instead).
    pub id: String,
    /// Declared `type`.
    pub note_type: String,
    /// Display title: the `title` frontmatter field, else the declared `id`.
    pub title: String,
    /// Declared `status`, normalised to lowercase. Defaults are applied by [`GraphNote::effective_status`].
    pub status: Option<String>,
    /// `project` field (decision/context: the project it mainly belongs to).
    pub project: Option<String>,
    /// `scope` field: other projects this note is also relevant to.
    pub scope: Vec<String>,
    /// `supersedes` field (decision only): ids of decisions this one replaces.
    pub supersedes: Vec<String>,
    /// `updated` or `last_verified`, whichever is declared (for sort order).
    pub updated: Option<String>,
}

impl GraphNote {
    /// `status`, defaulted to `active` when not declared (every type's active-like state).
    pub fn effective_status(&self) -> &str {
        self.status.as_deref().unwrap_or("active")
    }

    /// Whether this note is linked to `project_id`, directly or via `scope`.
    pub fn belongs_to(&self, project_id: &str) -> bool {
        self.project
            .as_deref()
            .is_some_and(|project| project.eq_ignore_ascii_case(project_id))
            || self
                .scope
                .iter()
                .any(|scope| scope.eq_ignore_ascii_case(project_id))
    }

    /// `true` when related via `scope` only (not the direct `project` owner) — rendered `(shared)`.
    pub fn is_shared_only(&self, project_id: &str) -> bool {
        !self
            .project
            .as_deref()
            .is_some_and(|project| project.eq_ignore_ascii_case(project_id))
            && self
                .scope
                .iter()
                .any(|scope| scope.eq_ignore_ascii_case(project_id))
    }
}

/// A note that declared `type` but could not be loaded into the graph, and why.
#[derive(Clone, Debug, Serialize)]
pub struct MalformedNote {
    pub path: String,
    pub note_type: String,
    pub reason: String,
}

/// Everything decodable from the index's declared metadata.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub notes: Vec<GraphNote>,
    pub malformed: Vec<MalformedNote>,
}

fn as_str_vec(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        _ => Vec::new(),
    }
}

fn as_str(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn is_template_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized.split('/').any(|part| part == "templates") || normalized.ends_with(".template.md")
}

/// Decode one stored document into a [`GraphNote`], or record why it was skipped.
fn decode(doc: &StoredDocument) -> Option<Result<GraphNote, String>> {
    // Templates contain placeholder ids, draft statuses, and example relationships. They are
    // searchable documents, but must never appear as live graph records.
    if is_template_path(&doc.path) {
        return None;
    }
    let note_type = as_str(doc.metadata.get("type"))?.to_lowercase();
    if NON_GRAPH_NOTE_TYPES.contains(&note_type.as_str()) {
        return None;
    }
    if !NOTE_TYPES.contains(&note_type.as_str()) {
        return Some(Err(format!(
            "unknown type '{note_type}' (expected one of: {})",
            NOTE_TYPES.join(", ")
        )));
    }
    let Some(id) = as_str(doc.metadata.get("id")) else {
        return Some(Err(format!(
            "type: {note_type} is missing a required 'id' field"
        )));
    };
    let status = as_str(doc.metadata.get("status")).map(|s| s.to_lowercase());
    if let Some(s) = &status {
        let allowed: &[&str] = match note_type.as_str() {
            "project" => &PROJECT_STATUSES,
            "decision" => &DECISION_STATUSES,
            "context" => &CONTEXT_STATUSES,
            _ => &[],
        };
        if !allowed.is_empty() && !allowed.contains(&s.as_str()) {
            return Some(Err(format!(
                "status '{s}' is not valid for type: {note_type} (expected one of: {})",
                allowed.join(", ")
            )));
        }
    }
    let title = as_str(doc.metadata.get("title")).unwrap_or_else(|| id.clone());
    let project = as_str(doc.metadata.get("project"));
    let scope = as_str_vec(doc.metadata.get("scope"));
    let supersedes = as_str_vec(doc.metadata.get("supersedes"));
    let updated =
        as_str(doc.metadata.get("updated")).or_else(|| as_str(doc.metadata.get("last_verified")));
    Some(Ok(GraphNote {
        path: doc.path.clone(),
        id,
        note_type,
        title,
        status,
        project,
        scope,
        supersedes,
        updated,
    }))
}

/// Load the project/decision/context graph from every indexed document.
///
/// Read-only; never fails the whole listing because of one bad note (see module docs).
pub async fn load_graph(db: &Database) -> crate::memory::error::Result<Graph> {
    let docs = db.all_documents().await?;
    let mut notes = Vec::new();
    let mut malformed = Vec::new();
    for doc in &docs {
        match decode(doc) {
            None => {}
            Some(Ok(note)) => notes.push(note),
            Some(Err(reason)) => {
                // `type` is always present when `decode` returns `Some`.
                let note_type = as_str(doc.metadata.get("type")).unwrap_or_default();
                malformed.push(MalformedNote {
                    path: doc.path.clone(),
                    note_type,
                    reason,
                });
            }
        }
    }
    notes.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.id.cmp(&b.id)));
    Ok(Graph { notes, malformed })
}

impl Graph {
    /// All `type: project` notes.
    pub fn projects(&self) -> Vec<&GraphNote> {
        self.notes
            .iter()
            .filter(|n| n.note_type == "project")
            .collect()
    }

    /// All notes of `note_type`.
    pub fn of_type(&self, note_type: &str) -> Vec<&GraphNote> {
        self.notes
            .iter()
            .filter(|n| n.note_type == note_type)
            .collect()
    }

    /// Ids of every active (or any-status) `type: project` note, duplicates removed, for
    /// ambiguity / "did you mean" messages.
    pub fn project_ids(&self) -> Vec<&str> {
        self.projects().iter().map(|p| p.id.as_str()).collect()
    }
}

/// Outcome of resolving a user-supplied project reference (id, id-prefix, or title).
pub enum Resolved<'a> {
    Found(&'a GraphNote),
    NotFound {
        query: String,
        suggestions: Vec<String>,
    },
    Ambiguous {
        query: String,
        candidates: Vec<String>,
    },
}

/// Resolve `query` against the graph's projects: exact id, exact title (case-insensitive),
/// else a unique id-prefix match. Ambiguous prefixes list every match; no match suggests the
/// closest titles/ids by substring.
pub fn resolve_project<'a>(graph: &'a Graph, query: &str) -> Resolved<'a> {
    let projects = graph.projects();
    let exact_ids: Vec<&&GraphNote> = projects
        .iter()
        .filter(|p| p.id.eq_ignore_ascii_case(query))
        .collect();
    match exact_ids.len() {
        1 => return Resolved::Found(exact_ids[0]),
        n if n > 1 => {
            return Resolved::Ambiguous {
                query: query.to_string(),
                candidates: exact_ids
                    .iter()
                    .map(|p| format!("{} ({})", p.id, p.title))
                    .collect(),
            };
        }
        _ => {}
    }
    let title_matches: Vec<&&GraphNote> = projects
        .iter()
        .filter(|p| p.title.eq_ignore_ascii_case(query))
        .collect();
    if title_matches.len() == 1 {
        return Resolved::Found(title_matches[0]);
    }
    if title_matches.len() > 1 {
        return Resolved::Ambiguous {
            query: query.to_string(),
            candidates: title_matches
                .iter()
                .map(|p| format!("{} ({})", p.id, p.title))
                .collect(),
        };
    }
    let prefix_matches: Vec<&&GraphNote> = projects
        .iter()
        .filter(|p| {
            p.id.get(..query.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(query))
        })
        .collect();
    match prefix_matches.len() {
        1 => Resolved::Found(prefix_matches[0]),
        n if n > 1 => Resolved::Ambiguous {
            query: query.to_string(),
            candidates: prefix_matches
                .iter()
                .map(|p| format!("{} ({})", p.id, p.title))
                .collect(),
        },
        _ => {
            let needle = query.to_lowercase();
            let suggestions: Vec<String> = projects
                .iter()
                .filter(|p| {
                    p.title.to_lowercase().contains(&needle)
                        || p.id.to_lowercase().contains(&needle)
                })
                .map(|p| format!("{} ({})", p.id, p.title))
                .collect();
            Resolved::NotFound {
                query: query.to_string(),
                suggestions,
            }
        }
    }
}

/// Filter applied by `decisions` / `contexts` (global or project-scoped) listings.
#[derive(Clone, Debug)]
pub struct ListFilter {
    /// Keep only notes linked (via `project` or `scope`) to one of these project ids. Empty
    /// means no project restriction. A project id that matches no project yields an empty
    /// (not an error) result — see module docs' note on filter vs. direct-target semantics.
    pub projects: Vec<String>,
    /// Keep only notes with one of these statuses. Empty means "apply the default visibility
    /// rule" (see `include_inactive`).
    pub status: Vec<String>,
    /// Also show notes whose status is normally hidden (superseded/revoked/archived/completed).
    pub include_inactive: bool,
    /// Maximum notes returned.
    pub limit: usize,
}

impl Default for ListFilter {
    fn default() -> Self {
        ListFilter {
            projects: Vec::new(),
            status: Vec::new(),
            include_inactive: false,
            limit: 50,
        }
    }
}

/// List every note of `note_type` matching `filter`.
///
/// Status visibility: with no explicit `--status` and no `--include-inactive`, notes whose
/// status is in that type's default-hidden set are left out, but the command never errors.
/// An explicit `--status` always wins (shows exactly those statuses, active or not).
pub fn list<'a>(graph: &'a Graph, note_type: &str, filter: &ListFilter) -> Vec<&'a GraphNote> {
    let hidden = inactive_statuses(note_type);
    let mut out: Vec<&GraphNote> = graph
        .of_type(note_type)
        .into_iter()
        .filter(|n| {
            if !filter.projects.is_empty() {
                let global_requested = filter.projects.iter().any(|p| p == "*");
                if !global_requested && !filter.projects.iter().any(|p| n.belongs_to(p)) {
                    return false;
                }
            }
            if !filter.status.is_empty() {
                return filter
                    .status
                    .iter()
                    .any(|status| status.eq_ignore_ascii_case(n.effective_status()));
            }
            if filter.include_inactive {
                return true;
            }
            !hidden.contains(&n.effective_status())
        })
        .collect();
    out.truncate(filter.limit.max(1));
    out
}

/// One "superseded by …" or conflicting-supersession warning, for display alongside a decision
/// listing. Read-only, never blocks the listing itself (see module docs).
#[derive(Clone, Debug, Serialize)]
pub struct SupersessionWarning {
    pub target_id: String,
    pub claimants: Vec<String>,
}

/// Decision ids claimed as superseded by more than one *active* decision — a data-hygiene
/// smell, not a listing failure. Also returns, for every decision that is itself superseded,
/// the id(s) of the decision(s) that replaced it (for "superseded by" labels).
pub fn supersession_map(graph: &Graph) -> (Vec<SupersessionWarning>, HashMap<String, Vec<String>>) {
    let mut claimants: HashMap<String, Vec<String>> = HashMap::new();
    for d in graph.of_type("decision") {
        if d.effective_status() != "active" {
            continue;
        }
        for target in &d.supersedes {
            claimants
                .entry(target.clone())
                .or_default()
                .push(d.id.clone());
        }
    }
    let conflicts = claimants
        .iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(target_id, v)| SupersessionWarning {
            target_id: target_id.clone(),
            claimants: v.clone(),
        })
        .collect();
    (conflicts, claimants)
}

#[cfg(test)]
#[path = "graph_test.rs"]
mod graph_test;
