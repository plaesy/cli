//! A dependency-free knowledge-graph builder for a project.
//!
//! It scans source, doc, and config files, extracts structural references
//! (Markdown links, dot-sourcing, imports), plain-text mentions, and per-file symbols,
//! runs label-propagation community detection, and emits `project.graph.json`.
//! Port of `scripts/internal/graph` (Go).

pub mod build;
pub mod community;
pub mod extensions;
pub mod extract;
pub mod incremental;
pub mod mentions;
pub mod query;
pub mod render;
pub mod scan;
pub mod symbols;
pub mod types;

pub use build::build_graph;
pub use types::{Edge, Graph, Node};

use std::path::{Path, PathBuf};

/// Failures of a graph build or of one of its readers and writers.
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("no files found under {}", .0.display())]
    NoFiles(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

/// Options that affect a build.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Project root.
    pub repo_path: PathBuf,
    /// Output folder, relative to the project root unless absolute. Empty means `.plaesy/analysis`.
    pub out_dir: String,
    /// Recompute every file, ignoring the cache.
    pub full: bool,
}

/// Resolved locations a build reads and writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub repo_root: PathBuf,
    /// Forward-slash form of the root, used for internal joins.
    pub repo_root_slash: String,
    pub out_full: PathBuf,
    pub project_json: PathBuf,
    pub fingerprint_file: PathBuf,
}

/// Resolves the paths for a build.
///
/// An absolute output folder is used as given. Joining it onto the root would
/// turn `/tmp/kg` into `<root>/tmp/kg`.
///
/// # Errors
///
/// The project root cannot be made absolute.
pub fn resolve_paths(opts: &Options) -> std::io::Result<Paths> {
    let repo_root = std::path::absolute(&opts.repo_path)?;
    let out_dir = if opts.out_dir.is_empty() {
        ".plaesy/analysis"
    } else {
        opts.out_dir.as_str()
    };
    let out_full = if Path::new(out_dir).is_absolute() {
        PathBuf::from(out_dir)
    } else {
        repo_root.join(out_dir)
    };
    Ok(Paths {
        repo_root_slash: scan::to_slash(&repo_root.to_string_lossy()),
        project_json: out_full.join("project.graph.json"),
        fingerprint_file: out_full.join(".fingerprint"),
        repo_root,
        out_full,
    })
}

/// The output folder relative to the root, forward-slashed. Excluded from the scan.
#[must_use]
pub fn out_dir_norm(p: &Paths) -> String {
    match p.out_full.strip_prefix(&p.repo_root) {
        Ok(rel) => scan::to_slash(&rel.to_string_lossy())
            .trim_end_matches('/')
            .to_owned(),
        Err(_) => String::new(),
    }
}
