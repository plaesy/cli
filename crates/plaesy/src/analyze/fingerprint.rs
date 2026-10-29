//! Fingerprints that decide whether a stored analysis is still valid, and which artifacts it built.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use super::output::{CFG_EXTS, DOC_EXTS, SOURCE_CODE_EXTS};
use super::walk::{mtime_secs, walk_project};
use crate::graph::{out_dir_norm, scan::collect_files, Paths};

/// Extensions whose change makes a stored analysis stale. Extra ones cover stylesheets, SQL,
/// lock files and editor files, which the counts do not classify.
const EXTRA_FINGERPRINT_EXTS: &[&str] = &[
    "sql",
    "scss",
    "sass",
    "less",
    "lock",
    "env",
    "gradle",
    "properties",
    "txt",
];

const FINGERPRINT_FILE: &str = ".analysis-fingerprint";
const COMPONENTS_TAG: &str = "|components=";

fn fingerprint_exts() -> BTreeSet<&'static str> {
    SOURCE_CODE_EXTS
        .iter()
        .chain(DOC_EXTS)
        .chain(CFG_EXTS)
        .chain(EXTRA_FINGERPRINT_EXTS)
        .copied()
        .collect()
}

/// The extension of a file name, lowercased, without the dot. `.env` counts as `env`.
fn ext_of(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.rsplit_once('.')
        .map_or(String::new(), |(_, ext)| ext.to_lowercase())
}

/// `<count>|<max mtime>|<version>` over the fingerprinted files of the project.
#[must_use]
pub fn compute_fingerprint(project: &Path, framework_version: &str) -> String {
    let exts = fingerprint_exts();
    let walk = walk_project(project).unwrap_or_default();
    let mut count = 0usize;
    let mut max_mtime = 0i64;
    for f in walk
        .files
        .iter()
        .filter(|f| exts.contains(ext_of(&f.rel_path).as_str()))
    {
        count += 1;
        max_mtime = max_mtime.max(f.mod_unix);
    }
    format!("{count}|{max_mtime}|{framework_version}")
}

/// The artifacts a run writes. The graph and symbols index are written only when the graph is on.
#[must_use]
pub fn built_components(no_graph: bool) -> Vec<&'static str> {
    if no_graph {
        vec!["structure"]
    } else {
        vec!["structure", "graph", "symbols"]
    }
}

fn with_components(fp: &str, components: &[&str]) -> String {
    if components.is_empty() {
        fp.to_owned()
    } else {
        format!("{fp}{COMPONENTS_TAG}{}", components.join(","))
    }
}

/// The walk fingerprint and the components a stored fingerprint recorded.
fn split_components(stored: &str) -> (&str, BTreeSet<&str>) {
    match stored.find(COMPONENTS_TAG) {
        None => (stored, BTreeSet::new()),
        Some(i) => (
            &stored[..i],
            stored[i + COMPONENTS_TAG.len()..].split(',').collect(),
        ),
    }
}

/// True when the last completed run matches the project and built every needed component.
#[must_use]
pub fn should_skip_analysis(
    project: &Path,
    analysis_dir: &Path,
    framework_version: &str,
    need: &[&str],
) -> bool {
    let Ok(stored) = fs::read_to_string(analysis_dir.join(FINGERPRINT_FILE)) else {
        return false;
    };
    let (base, built) = split_components(&stored);
    if base != compute_fingerprint(project, framework_version) {
        return false;
    }
    need.iter().all(|c| built.contains(c))
}

/// Stores the fingerprint of a finished analysis. Called only after every artifact is written,
/// because the fingerprint claims that they exist.
///
/// # Errors
///
/// The fingerprint file cannot be written.
pub fn record_fingerprint(
    project: &Path,
    analysis_dir: &Path,
    framework_version: &str,
    components: &[&str],
) -> io::Result<()> {
    let fp = with_components(&compute_fingerprint(project, framework_version), components);
    fs::write(analysis_dir.join(FINGERPRINT_FILE), fp)
}

/// A cheap "did the sources change" signature: the file count and newest mtime over the same set
/// the graph scans.
///
/// # Errors
///
/// The tree cannot be walked.
pub fn source_fingerprint(p: &Paths) -> io::Result<String> {
    let files = collect_files(&p.repo_root, &out_dir_norm(p))?;
    let mut newest = 0i64;
    for rel in &files {
        let abs = crate::graph::scan::join_root(&p.repo_root_slash, rel);
        if let Ok(meta) = fs::metadata(abs) {
            newest = newest.max(mtime_secs(&meta));
        }
    }
    Ok(format!("{}|{newest}", files.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerprint_without_components_satisfies_no_need() {
        let (base, built) = split_components("3|10|1.0.0");
        assert_eq!(base, "3|10|1.0.0");
        assert!(built.is_empty());
    }

    #[test]
    fn components_round_trip() {
        let fp = with_components("3|10|1.0.0", &["structure", "graph"]);
        let (base, built) = split_components(&fp);
        assert_eq!(base, "3|10|1.0.0");
        assert!(built.contains("graph"));
        assert!(!built.contains("symbols"));
    }

    #[test]
    fn dotenv_counts_as_an_env_extension() {
        assert_eq!(ext_of(".env"), "env");
        assert_eq!(ext_of("a/b.TXT"), "txt");
        assert_eq!(ext_of("Makefile"), "");
    }
}
