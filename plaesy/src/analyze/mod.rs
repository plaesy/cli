//! Project analysis: writes the `.plaesy/analysis/` artifacts. Port of `scripts/internal/analyze` (Go).
//!
//! Artifacts: `project.json`, `project.structure.json`, `overview.md`, the dependency graph
//! (`project.graph.json`, `project.html`, `reports.md`), and `project.symbols.md`.
//!
//! The Go version also wrote a document index (`docs/`) for the removed `search` command. That
//! index is not ported.

pub mod detect;
pub mod fingerprint;
pub mod insights;
pub mod output;
pub mod walk;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::log::Logger;
use crate::graph::{self, render, GraphError};

use detect::{
    detect_all_frameworks, detect_all_languages, detect_build_systems, detect_development_tools,
    detect_project_type,
};
use fingerprint::{built_components, record_fingerprint, should_skip_analysis, source_fingerprint};
use output::{
    count_file_types, write_overview, write_project_json, write_project_structure_json, FileCounts,
    RunContext,
};
use walk::walk_project;

/// Options for one analysis run.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Project root. Empty means the working directory.
    pub project_path: Option<PathBuf>,
    /// Skip the dependency graph and the symbols index.
    pub no_graph: bool,
    /// Regenerate even when the fingerprint says nothing changed.
    pub force: bool,
    /// Accepted for the legacy `--if-changed` flag. The fingerprint fast path is always on.
    pub if_changed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum AnalyzeError {
    #[error("walk project: {0}")]
    Walk(std::io::Error),
    #[error("generate {0}: {1}")]
    Artifact(&'static str, std::io::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Graph(#[from] GraphError),
}

fn framework_version() -> String {
    let v = crate::version();
    if v.is_empty() || v == "0.0.0" {
        "0.0.1".to_owned()
    } else {
        v.to_owned()
    }
}

fn project_root(opts: &Options) -> std::io::Result<PathBuf> {
    match &opts.project_path {
        Some(p) => std::path::absolute(p),
        None => std::env::current_dir(),
    }
}

/// Runs the analysis once. Writes the artifacts, or skips when the fingerprint still matches.
///
/// # Errors
///
/// The project cannot be walked, or a summary artifact cannot be written. A failed graph build
/// is reported, not returned: the summary artifacts stay valid.
pub fn run(opts: &Options, log: &Logger) -> Result<(), AnalyzeError> {
    let project = project_root(opts)?;
    let analysis_dir = project.join(".plaesy").join("analysis");
    fs::create_dir_all(&analysis_dir)?;
    fs::create_dir_all(project.join(".plaesy").join("memory"))?;

    let version = framework_version();
    log.info("Starting comprehensive project analysis...");
    log.info(&format!("Project path: {}", project.display()));
    log.info(&format!("Analysis directory: {}", analysis_dir.display()));

    let components = built_components(opts.no_graph);
    if !opts.force && should_skip_analysis(&project, &analysis_dir, &version, &components) {
        log.success(
            "Analysis unchanged since last run. Skipping regeneration (use --force to override).",
        );
        log.info(&format!("Analysis files (in {}):", analysis_dir.display()));
        log.info("   - project.json - AI-optimized project summary (cached)");
        log.info("   - project.structure.json - Detailed project structure (cached)");
        log.info("   - overview.md - Analysis snapshot (cached)");
        if !opts.no_graph {
            log.info("   - project.graph.json - Dependency graph (cached)");
            log.info("   - project.html - Interactive graph visualization (cached)");
            log.info("   - reports.md - Graph report (cached)");
            log.info("   - project.symbols.md - Function/class index (cached)");
        }
        return Ok(());
    }

    let walk = walk_project(&project).map_err(AnalyzeError::Walk)?;
    let frameworks = detect_all_frameworks(&project);
    let project_type = detect_project_type(&frameworks);
    let languages = detect_all_languages(
        &walk
            .files
            .iter()
            .map(|f| f.rel_path.clone())
            .collect::<Vec<_>>(),
    );
    let dev_tools = detect_development_tools(&project);
    let build_systems = detect_build_systems(&project);
    let file_counts: FileCounts = count_file_types(&walk.files);

    let ctx = RunContext {
        project: &project,
        walk: &walk,
        frameworks: &frameworks,
        project_type: &project_type,
        languages: &languages,
        dev_tools: &dev_tools,
        build_systems: &build_systems,
        file_counts,
        framework_version: &version,
    };
    write_project_json(&analysis_dir, &ctx)
        .map_err(|e| AnalyzeError::Artifact("project.json", e))?;
    write_project_structure_json(&analysis_dir, &ctx)
        .map_err(|e| AnalyzeError::Artifact("project.structure.json", e))?;
    write_overview(&analysis_dir, &ctx).map_err(|e| AnalyzeError::Artifact("overview.md", e))?;
    log.success("Comprehensive project.json generated");
    log.success("Detailed project.structure.json generated");
    log.success("analysis/overview.md replaced with latest snapshot");

    let mut graph_failed = false;
    if !opts.no_graph {
        if opts.force {
            log.info("Building dependency graph (forced)...");
        } else {
            log.info("Building dependency graph (full rebuild, runs only when source changed)...");
        }
        if let Err(err) = build_graph_artifacts(&project, opts.force, log) {
            // A failed graph build is not fatal. The summary files stay valid, and the fingerprint
            // must not claim a graph that was never written.
            log.info(&format!(
                "Graph build skipped/failed; summary files remain valid. ({err})"
            ));
            graph_failed = true;
        } else if let Err(err) = generate_symbols_index(&project, &analysis_dir) {
            log.info(&format!(
                "Symbols index skipped/failed; other analysis files remain valid. ({err})"
            ));
            graph_failed = true;
        } else {
            log.success(
                "analysis/project.symbols.md generated (function/class index, grouped by file)",
            );
        }
    }

    log.success("Comprehensive analysis completed!");

    if opts.no_graph {
        log.info("Not recording the analysis fingerprint: --no-graph built no graph, so the next run must build it.");
    } else if graph_failed {
        log.info("Not recording the analysis fingerprint: the graph artifacts are missing, so the next run must regenerate.");
    } else if let Err(err) =
        record_fingerprint(&project, &analysis_dir, &version, &built_components(false))
    {
        log.info(&format!(
            "Could not record the analysis fingerprint (a later run will regenerate): {err}"
        ));
    }

    log.info("Generated files:");
    log.info(&format!("Analysis files (in {}):", analysis_dir.display()));
    log.info("   - project.json - AI-optimized project summary");
    log.info("   - project.structure.json - Detailed project structure");
    if !opts.no_graph && !graph_failed {
        log.info("   - project.graph.json - Dependency graph (nodes + edges)");
        log.info("   - project.html - Interactive graph visualization");
        log.info("   - reports.md - Graph report (communities, god nodes, orphans)");
        log.info("   - project.symbols.md - Function/class index, grouped by file (check before adding new functions)");
    }
    log.info("   - overview.md - Analysis snapshot (replaced every run)");
    log.info("No topic memory files are generated by analyze.");
    Ok(())
}

/// Builds the graph and writes its JSON, HTML and report, unless the sources are unchanged.
fn build_graph_artifacts(project: &Path, force: bool, log: &Logger) -> Result<(), AnalyzeError> {
    let opts = graph::Options {
        repo_path: project.to_path_buf(),
        out_dir: String::new(),
        full: force,
    };
    let paths = graph::resolve_paths(&opts)?;
    if !force {
        if let Ok(fp) = source_fingerprint(&paths) {
            if fs::read_to_string(&paths.fingerprint_file).is_ok_and(|prev| prev == fp) {
                return Ok(());
            }
        }
    }
    let g = graph::build_graph(&opts, &paths, log)?;
    render::save_json(&g, &paths.project_json)?;
    render::write_html(&g, &paths.out_full)?;
    render::write_report(&g, &paths.out_full, "")?;
    if let Ok(fp) = source_fingerprint(&paths) {
        fs::write(&paths.fingerprint_file, fp)?;
    }
    Ok(())
}

/// Writes `project.symbols.md`: every symbol the graph extracted, grouped by file, with
/// duplicate and near-duplicate names called out. It carries no line numbers, since those go
/// stale on every edit.
fn generate_symbols_index(project: &Path, analysis_dir: &Path) -> Result<(), AnalyzeError> {
    let paths = graph::resolve_paths(&graph::Options {
        repo_path: project.to_path_buf(),
        ..Default::default()
    })?;
    let g = render::load_json(&paths.project_json)?;

    let mut files: Vec<(&str, &[String])> = g
        .nodes
        .iter()
        .filter(|n| !n.symbols.is_empty())
        .map(|n| (n.id.as_str(), n.symbols.as_slice()))
        .collect();
    files.sort_by(|a, b| a.0.cmp(b.0));

    // Markdown headings repeat across docs by design, so they are not duplicates.
    let mut by_name: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (path, symbols) in &files {
        if Path::new(path).extension().is_some_and(|e| e == "md") {
            continue;
        }
        for s in *symbols {
            by_name.entry(s.as_str()).or_default().push(path);
        }
    }
    let dup_names: Vec<&str> = by_name
        .iter()
        .filter(|(_, locs)| locs.len() > 1)
        .map(|(n, _)| *n)
        .collect();

    let mut by_normalized: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for name in by_name.keys() {
        by_normalized
            .entry(normalize_symbol_name(name))
            .or_default()
            .push(name);
    }
    let similar: Vec<Vec<&str>> = by_normalized
        .values()
        .filter(|spellings| spellings.len() > 1)
        .map(|spellings| {
            let mut s = spellings.clone();
            s.sort_unstable();
            s
        })
        .collect();

    let mut b = String::new();
    b.push_str("# Symbols Index\n\n");
    b.push_str("Function/class/heading names extracted per file, for duplicate-avoidance checks before adding new code. ");
    b.push_str("No line numbers: they go stale on every edit. Regenerated every `plaesy analyze` run alongside the dependency graph.\n\n");
    if files.is_empty() {
        b.push_str("_No symbols extracted for this project's current file set._\n");
    }

    b.push_str("## Duplicate Names (appear more than once)\n\n");
    if dup_names.is_empty() {
        b.push_str("_None found._\n\n");
    } else {
        b.push_str("Same name more than once: could be intentional (interface implementations, per-package `New`, overloads in languages that support them) or an accidental duplicate — check before adding another one.\n\n");
        for name in &dup_names {
            b.push_str(&format!(
                "- `{name}` — {}\n",
                format_locations(&by_name[name])
            ));
        }
        b.push('\n');
    }

    b.push_str("## Similar Names (different spelling, same normalized form)\n\n");
    if similar.is_empty() {
        b.push_str("_None found._\n\n");
    } else {
        b.push_str("Same letters/digits once case and separators (`_`) are stripped — likely the same concept named inconsistently (or copy-pasted and renamed). Not caught by exact-name matching above.\n\n");
        for spellings in &similar {
            let parts: Vec<String> = spellings
                .iter()
                .map(|name| format!("`{name}` ({})", format_locations(&by_name[name])))
                .collect();
            b.push_str(&format!("- {}\n", parts.join(", ")));
        }
        b.push('\n');
    }

    for (path, symbols) in &files {
        b.push_str(&format!("## {path}\n\n"));
        for s in *symbols {
            b.push_str(&format!("- `{s}`\n"));
        }
        b.push('\n');
    }

    fs::write(analysis_dir.join("project.symbols.md"), b)?;
    Ok(())
}

/// Lowercase letters and digits only, so `getUserById`, `get_user_by_id` and `GetUserById` match.
#[must_use]
pub fn normalize_symbol_name(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Locations joined by comma. A file listed several times shows as `path (xN)`, not as N entries.
#[must_use]
pub fn format_locations(paths: &[&str]) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for p in paths {
        *counts.entry(p).or_insert(0) += 1;
    }
    counts
        .iter()
        .map(|(p, n)| {
            if *n > 1 {
                format!("{p} (x{n})")
            } else {
                (*p).to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Runs the analysis, then re-runs it whenever the graph sources change.
///
/// # Errors
///
/// A run fails, or the source fingerprint cannot be read.
pub fn watch(opts: &Options, interval: Duration, log: &Logger) -> Result<(), AnalyzeError> {
    run(opts, log)?;
    let repo = opts
        .project_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let paths = graph::resolve_paths(&graph::Options {
        repo_path: repo,
        ..Default::default()
    })?;
    let mut last = source_fingerprint(&paths)?;

    log.info(&format!(
        "Watch mode: polling every {interval:?}. Ctrl+C to stop."
    ));
    loop {
        std::thread::sleep(interval);
        let fp = source_fingerprint(&paths)?;
        if fp == last {
            continue;
        }
        log.info("Change detected, re-running analyze...");
        run(opts, log)?;
        last = source_fingerprint(&paths)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_names_normalize_across_conventions() {
        assert_eq!(
            normalize_symbol_name("getUserById"),
            normalize_symbol_name("get_user_by_id")
        );
        assert_eq!(normalize_symbol_name("GetUserById"), "getuserbyid");
    }

    #[test]
    fn repeated_locations_collapse_with_a_count() {
        assert_eq!(
            format_locations(&["a.go", "a.go", "b.go"]),
            "a.go (x2), b.go"
        );
    }

    #[test]
    fn a_run_writes_the_summary_artifacts() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
        let opts = Options {
            project_path: Some(dir.path().to_path_buf()),
            no_graph: true,
            force: true,
            if_changed: false,
        };
        let log = Logger::new("test");
        run(&opts, &log).expect("run");
        let analysis = dir.path().join(".plaesy").join("analysis");
        assert!(analysis.join("project.json").is_file());
        assert!(analysis.join("project.structure.json").is_file());
        assert!(analysis.join("overview.md").is_file());
    }
}
