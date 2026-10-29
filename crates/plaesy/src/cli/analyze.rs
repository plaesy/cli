//! `plaesy analyze`: writes `.plaesy/analysis/` and answers graph queries from the graph on disk.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::common::logger;
use super::CliResult;
use crate::analyze::{self, Options};
use crate::graph::{self, query, render, Paths};

/// Writes the analysis. The query flags answer from the graph already on disk and do not
/// regenerate the analysis, so run `analyze` once without them first.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Project to analyze (default: current directory)
    pub project_path: Option<PathBuf>,
    /// Skip the dependency graph build entirely
    #[arg(long)]
    pub no_graph: bool,
    /// Regenerate even when the project fingerprint is unchanged
    #[arg(long)]
    pub force: bool,
    /// Legacy no-op: the fingerprint fast path is always on
    #[arg(long, hide = true)]
    pub if_changed: bool,
    /// Re-run analyze automatically when source files change
    #[arg(long)]
    pub watch: bool,
    /// Poll interval in seconds for --watch
    #[arg(long, default_value_t = 3)]
    pub watch_interval: u64,

    /// Keyword query against node ids
    #[arg(long)]
    pub query: Option<String>,
    /// Fuzzy/substring search against node ids
    #[arg(long)]
    pub fuzzy_query: Option<String>,
    /// Search depth for --fuzzy-query (display only)
    #[arg(long, default_value_t = 2)]
    pub search_depth: usize,
    /// Shortest-path query: source node
    #[arg(long)]
    pub path_query_from: Option<String>,
    /// Shortest-path query: target node
    #[arg(long)]
    pub path_query_to: Option<String>,
    /// Explain one node's edges and symbols
    #[arg(long)]
    pub explain: Option<String>,
    /// List nodes impacted by changing this node
    #[arg(long)]
    pub impact_check: Option<String>,
    /// BFS depth for --impact-check
    #[arg(long, default_value_t = 2)]
    pub impact_depth: usize,
    /// Write impact-visualization.html instead of printing
    #[arg(long)]
    pub impact_visualize: bool,
    /// Export INFERRED edges to semantic-queue.json
    #[arg(long)]
    pub semantic_queue: bool,
    /// Export INFERRED edges to business-logic-queue.json
    #[arg(long)]
    pub business_logic: bool,
    /// Write a learning-paths.json stub
    #[arg(long)]
    pub generate_paths: bool,
    /// Merge a {source,target,rationale}[] annotations file into reports.md
    #[arg(long)]
    pub apply_semantic: Option<PathBuf>,
}

impl Args {
    fn has_query(&self) -> bool {
        self.query.is_some()
            || self.fuzzy_query.is_some()
            || self.path_query_from.is_some() && self.path_query_to.is_some()
            || self.explain.is_some()
            || self.impact_check.is_some()
            || self.semantic_queue
            || self.business_logic
            || self.generate_paths
            || self.apply_semantic.is_some()
    }
}

pub fn run(args: &Args) -> CliResult {
    let log = logger();
    if args.has_query() {
        let repo = args
            .project_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("."));
        let paths = graph::resolve_paths(&graph::Options {
            repo_path: repo,
            out_dir: ".plaesy/analysis".to_owned(),
            full: false,
        })?;
        return run_query(args, &paths);
    }

    let opts = Options {
        project_path: args.project_path.clone(),
        no_graph: args.no_graph,
        force: args.force,
        if_changed: args.if_changed,
    };
    if args.watch {
        analyze::watch(&opts, Duration::from_secs(args.watch_interval), &log)?;
    } else {
        analyze::run(&opts, &log)?;
    }
    Ok(())
}

/// Loads the graph on disk, or explains how to build it.
fn load_graph(paths: &Paths) -> Result<crate::graph::Graph, Box<dyn std::error::Error>> {
    if !paths.project_json.exists() {
        return Err(format!(
            "no graph found in {}. Run `plaesy analyze` without query flags first to build it",
            paths.out_full.display()
        )
        .into());
    }
    Ok(render::load_json(&paths.project_json)?)
}

fn run_query(args: &Args, paths: &Paths) -> CliResult {
    let stdout = io::stdout();
    let log = logger();
    if let Some(node) = &args.explain {
        let g = load_graph(paths)?;
        query::do_explain(&g, node, &mut stdout.lock(), &log)?;
    } else if let Some(node) = &args.impact_check {
        let g = load_graph(paths)?;
        if args.impact_visualize {
            eprintln!("[plaesy-analyze] Generating impact visualization for '{node}'...");
            graph::query::write_impact_visualization(node, &paths.out_full)?;
            eprintln!("[plaesy-analyze] Impact visualization written to impact-visualization.html");
        } else {
            query::do_impact_check(&g, node, args.impact_depth, &mut stdout.lock(), &log)?;
        }
    } else if let (Some(from), Some(to)) = (&args.path_query_from, &args.path_query_to) {
        let g = load_graph(paths)?;
        query::do_path_query(&g, from, to, &mut stdout.lock(), &log)?;
    } else if args.business_logic {
        let g = load_graph(paths)?;
        let count = query::write_business_logic_queue(&g, &paths.out_full)?;
        eprintln!(
            "[plaesy-analyze] Generating business logic queue with {count} inferred edges..."
        );
        eprintln!(
            "[plaesy-analyze] Business logic queue written to {}",
            paths.out_full.join("business-logic-queue.json").display()
        );
    } else if args.generate_paths {
        let g = load_graph(paths)?;
        eprintln!("[plaesy-analyze] Generating learning paths via topological analysis...");
        query::write_learning_paths(&g, &paths.out_full)?;
        eprintln!(
            "[plaesy-analyze] Learning paths written to {}",
            paths.out_full.join("learning-paths.json").display()
        );
    } else if let Some(text) = &args.fuzzy_query {
        let g = load_graph(paths)?;
        eprintln!(
            "[plaesy-analyze] Fuzzy search for '{text}' (search depth: {}):",
            args.search_depth
        );
        query::do_fuzzy_query(&g, text, &mut io::stderr().lock())?;
        eprintln!();
    } else if args.semantic_queue {
        let g = load_graph(paths)?;
        let count = query::write_semantic_queue(&g, &paths.out_full)?;
        if count == 0 {
            eprintln!("[plaesy-analyze] No INFERRED edges to review - nothing queued.");
        } else {
            eprintln!(
                "[plaesy-analyze] Wrote {count} INFERRED edges to {}",
                paths.out_full.join("semantic-queue.json").display()
            );
            eprintln!(
                "[plaesy-analyze] Ask your AI assistant to explain each pair, save the result as"
            );
            eprintln!(
                "  a JSON array of {{source,target,rationale}}, then run --apply-semantic <file>."
            );
        }
    } else if let Some(file) = &args.apply_semantic {
        let g = load_graph(paths)?;
        if !Path::new(file).exists() {
            // A missing input file is a user error, not an empty result.
            return Err(format!("annotations file not found: {}", file.display()).into());
        }
        let applied = render::apply_semantic(&g, file, &paths.out_full, &log)?;
        eprintln!("[plaesy-analyze] Applied rationale to {applied} edge(s). reports.md updated.");
    } else if let Some(text) = &args.query {
        let g = load_graph(paths)?;
        query::do_query(&g, text, &mut stdout.lock(), &log)?;
    }
    io::stdout().flush()?;
    Ok(())
}
