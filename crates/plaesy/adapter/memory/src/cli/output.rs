//! Terminal output helpers for `plaesy memory` commands.

use super::definitions::*;
use crate::memory::curation::CurationReport;
use crate::memory::{MemoryError, Result};
use std::path::PathBuf;

pub(super) fn parse_project_roots(
    values: &[String],
) -> Result<std::collections::BTreeMap<String, Vec<PathBuf>>> {
    let mut roots = std::collections::BTreeMap::new();
    for value in values {
        let Some((slug, path)) = value.split_once('=') else {
            return Err(MemoryError::ConfigError(format!(
                "invalid --project-root {value:?}; expected SLUG=PATH"
            )));
        };
        let slug = slug.trim();
        let path = path.trim();
        if slug.is_empty()
            || slug == "."
            || slug == ".."
            || slug.contains(['/', '\\'])
            || path.is_empty()
        {
            return Err(MemoryError::ConfigError(format!(
                "invalid --project-root {value:?}; use a project slug and non-empty path"
            )));
        }
        let path = PathBuf::from(path);
        if !path.is_dir() {
            return Err(MemoryError::ConfigError(format!(
                "project root for {slug:?} is not a directory: {}",
                path.display()
            )));
        }
        roots
            .entry(slug.to_owned())
            .or_insert_with(Vec::new)
            .push(path);
    }
    Ok(roots)
}

/// `--project` is a *filter*: an id matching no project yields an empty result (see graph
/// module docs), except `*` which always means "every project" and bypasses this check.
pub(super) fn validate_projects_filter(
    graph: &crate::memory::graph::Graph,
    projects: &[String],
) -> Result<()> {
    for p in projects {
        if p == "*" {
            continue;
        }
        if !graph.project_ids().contains(&p.as_str()) {
            eprintln!("note: no project matches '{p}'; it will filter out everything for that id");
        }
    }
    Ok(())
}

pub(super) fn print_projects(
    graph: &crate::memory::graph::Graph,
    include_inactive: bool,
    format: GraphFormat,
) {
    let filter = crate::memory::graph::ListFilter {
        include_inactive,
        limit: usize::MAX,
        ..Default::default()
    };
    let items = crate::memory::graph::list(graph, "project", &filter);
    match format {
        GraphFormat::Json => print_json(&items),
        GraphFormat::Text => {
            if items.is_empty() {
                println!("No projects found. Create one with `type: project` frontmatter.");
                return;
            }
            for p in items {
                let decisions = graph
                    .of_type("decision")
                    .iter()
                    .filter(|d| d.belongs_to(&p.id))
                    .count();
                let contexts = graph
                    .of_type("context")
                    .iter()
                    .filter(|c| c.belongs_to(&p.id))
                    .count();
                println!(
                    "{:<24} {:<10} {:<28} decisions={decisions} contexts={contexts}",
                    p.id,
                    p.effective_status(),
                    p.title
                );
            }
        }
    }
}

pub(super) fn print_project_detail(
    graph: &crate::memory::graph::Graph,
    id: &str,
    format: GraphFormat,
) -> Result<()> {
    if id == "*" {
        return Err(MemoryError::ConfigError(
            "\"*\" is only valid before a subcommand, e.g. `projects * decisions`. Use `projects` to list all.".into(),
        ));
    }
    use crate::memory::graph::Resolved;
    match crate::memory::graph::resolve_project(graph, id) {
        Resolved::Found(p) => {
            let decisions: Vec<_> = graph
                .of_type("decision")
                .into_iter()
                .filter(|d| d.belongs_to(&p.id))
                .collect();
            let contexts: Vec<_> = graph
                .of_type("context")
                .into_iter()
                .filter(|c| c.belongs_to(&p.id))
                .collect();
            match format {
                GraphFormat::Json => print_json(&serde_json::json!({
                    "id": p.id, "title": p.title, "status": p.effective_status(),
                    "decisions_count": decisions.len(), "contexts_count": contexts.len(),
                })),
                GraphFormat::Text => {
                    println!("{}  ({})", p.title, p.id);
                    println!("  status:    {}", p.effective_status());
                    println!("  decisions: {}", decisions.len());
                    println!("  contexts:  {}", contexts.len());
                }
            }
            Ok(())
        }
        Resolved::Ambiguous { query, candidates } => Err(MemoryError::ConfigError(format!(
            "\"{query}\" matches {} projects: {}. Use the full id.",
            candidates.len(),
            candidates.join(", ")
        ))),
        Resolved::NotFound { query, suggestions } => {
            if suggestions.is_empty() {
                Err(MemoryError::ConfigError(format!(
                    "no project found with id \"{query}\". Run `memory projects` to list all."
                )))
            } else {
                Err(MemoryError::ConfigError(format!(
                    "no project found with id \"{query}\". Did you mean: {}? Run `memory projects` to list all.",
                    suggestions.join(", ")
                )))
            }
        }
    }
}

pub(super) fn print_project_scoped(
    graph: &crate::memory::graph::Graph,
    id: &str,
    sub: ProjectsSubcommand,
    _format: GraphFormat,
) -> Result<()> {
    use crate::memory::graph::{ListFilter, Resolved};
    let (note_type, include_inactive, status, limit, format) = match sub {
        ProjectsSubcommand::Decisions {
            include_inactive,
            status,
            limit,
            format,
        } => ("decision", include_inactive, status, limit, format),
        ProjectsSubcommand::Contexts {
            include_inactive,
            status,
            limit,
            format,
        } => ("context", include_inactive, status, limit, format),
    };
    let projects = if id == "*" {
        Vec::new() // no restriction: same as the unfiltered global command
    } else {
        match crate::memory::graph::resolve_project(graph, id) {
            Resolved::Found(p) => vec![p.id.clone()],
            Resolved::Ambiguous { query, candidates } => {
                return Err(MemoryError::ConfigError(format!(
                    "\"{query}\" matches {} projects: {}. Use the full id.",
                    candidates.len(),
                    candidates.join(", ")
                )));
            }
            Resolved::NotFound { query, suggestions } => {
                return Err(MemoryError::ConfigError(if suggestions.is_empty() {
                    format!(
                        "no project found with id \"{query}\". Run `memory projects` to list all."
                    )
                } else {
                    format!(
                        "no project found with id \"{query}\". Did you mean: {}? Run `memory projects` to list all.",
                        suggestions.join(", ")
                    )
                }));
            }
        }
    };
    let filter = ListFilter {
        projects,
        status,
        include_inactive,
        limit,
    };
    match note_type {
        "decision" => print_decision_list(graph, &filter, format),
        _ => print_context_list(graph, &filter, format),
    }
    Ok(())
}

pub(super) fn print_decision_list(
    graph: &crate::memory::graph::Graph,
    filter: &crate::memory::graph::ListFilter,
    format: GraphFormat,
) {
    let items = crate::memory::graph::list(graph, "decision", filter);
    let (conflicts, superseded_by) = crate::memory::graph::supersession_map(graph);
    match format {
        GraphFormat::Json => print_json(&items),
        GraphFormat::Text => {
            if items.is_empty() {
                let scope =
                    if filter.projects.is_empty() || filter.projects.iter().any(|p| p == "*") {
                        "".to_string()
                    } else {
                        format!(" for project \"{}\"", filter.projects.join(", "))
                    };
                println!("No decisions{scope}.");
                return;
            }
            for d in &items {
                let owner = d.project.as_deref().unwrap_or("[global]");
                let shared = if filter.projects.iter().any(|p| d.is_shared_only(p)) {
                    " (shared)"
                } else {
                    ""
                };
                print!("[{owner}]{shared}  {}  {}", d.title, d.effective_status());
                if let Some(by) = superseded_by.get(&d.id) {
                    print!("  superseded by: {}", by.join(", "));
                }
                println!();
            }
            for c in &conflicts {
                eprintln!(
                    "⚠ conflicting supersession: \"{}\" is claimed superseded by {} — run `memory hygiene --strict`",
                    c.target_id,
                    c.claimants.join(", ")
                );
            }
        }
    }
}

pub(super) fn print_context_list(
    graph: &crate::memory::graph::Graph,
    filter: &crate::memory::graph::ListFilter,
    format: GraphFormat,
) {
    let items = crate::memory::graph::list(graph, "context", filter);
    match format {
        GraphFormat::Json => print_json(&items),
        GraphFormat::Text => {
            if items.is_empty() {
                println!("No contexts.");
                return;
            }
            for c in &items {
                let owner = c.project.as_deref().unwrap_or("[global]");
                println!("[{owner}]  {}  {}", c.title, c.effective_status());
            }
        }
    }
}

pub(super) fn print_curation_report(report: &CurationReport) {
    println!(
        "Memory curation review: {} notes checked (read-only)",
        report.checked
    );
    println!(
        "Hygiene findings: {} total ({} confirmed, {} inferred)",
        report.hygiene_findings_total, report.confirmed_findings, report.inferred_findings
    );
    for (kind, count) in &report.hygiene.summary {
        println!("  {kind}: {count}");
        for finding in report
            .hygiene
            .findings
            .iter()
            .filter(|finding| &finding.kind == kind)
            .take(3)
        {
            println!("    {} — {}", finding.path, finding.detail);
        }
    }
    if report.hygiene_has_more {
        println!("  more hygiene findings omitted; use `memory hygiene` or increase --limit");
    }

    println!("Unanswered searches: {}", report.query_gaps.items.len());
    for gap in &report.query_gaps.items {
        println!("  #{} ({} misses) {}", gap.id, gap.misses, gap.query);
    }
    if report.query_gaps.has_more {
        println!("  more query gaps omitted; increase --limit to show more");
    }

    println!(
        "Exact duplicate groups: {}",
        report.exact_duplicates.items.len()
    );
    for group in &report.exact_duplicates.items {
        println!("  {}", group.files.items.join(" = "));
        if group.files.has_more {
            println!("    additional duplicate paths omitted");
        }
    }
    if report.exact_duplicates.has_more {
        println!("  more duplicate groups omitted; increase --limit to show more");
    }

    println!(
        "Unlinked note candidates: {}",
        report.unlinked_notes.items.len()
    );
    for path in &report.unlinked_notes.items {
        println!("  {path}");
    }
    if report.unlinked_notes.has_more {
        println!("  more unlinked notes omitted; increase --limit to show more");
    }

    println!(
        "Knowledge folders that may need an index ({} direct notes threshold): {}",
        report.topic_index_threshold,
        report.missing_topic_indexes.items.len()
    );
    for candidate in &report.missing_topic_indexes.items {
        println!("  {} ({} notes)", candidate.folder, candidate.notes);
    }
    if report.missing_topic_indexes.has_more {
        println!("  more folders omitted; increase --limit to show more");
    }
    println!("Review candidates manually; this command does not move, edit, or delete notes.");
}

pub(super) fn print_json(v: &impl serde::Serialize) {
    match serde_json::to_string_pretty(v) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("error: failed to serialize: {e}"),
    }
}

pub(super) fn require_dir(path: &std::path::Path) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(MemoryError::ConfigError(format!(
            "{} is not a directory",
            path.display()
        )))
    }
}
