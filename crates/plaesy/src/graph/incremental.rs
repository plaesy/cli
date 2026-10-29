//! Build cache: a file whose size, mtime and content hash are unchanged keeps its results.
//!
//! The cache only saves work. A build with or without it gives the same graph.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::common::log::Logger;

use super::extract::{find_files_named, ExtractContext};
use super::mentions;
use super::scan::{join_root, read_all};
use super::symbols::symbols_of;
use super::types::Edge;
use super::{Options, Paths};

/// Bump when a file's results change for the same content: a new extractor, or a changed rule.
pub const BUILD_CACHE_VERSION: u32 = 1;
pub const BUILD_CACHE_NAME: &str = "graph.cache.json";

/// One file's results from the last build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFile {
    pub size: u64,
    pub mtime_ns: i64,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
}

/// What the last build computed, and what it was computed against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildCache {
    pub version: u32,
    pub files: Vec<String>,
    pub manifest: String,
    pub entries: BTreeMap<String, CachedFile>,
}

/// The per-file results of one build.
#[derive(Debug, Default)]
pub struct FileResults {
    pub edges: Vec<Edge>,
    pub symbols: BTreeMap<String, Vec<String>>,
    /// Files whose results were recomputed this run. Unchanged files count zero.
    pub recomputed: usize,
}

/// Structural, mention and symbol results for every file.
///
/// A file's results depend on its content, on the node set, and on the `go.mod` and
/// `pubspec.yaml` manifests. A change to the node set or a manifest recomputes every file.
/// Otherwise only files whose content changed are recomputed.
pub fn file_results(
    opts: &Options,
    p: &Paths,
    files: &[String],
    known: &BTreeSet<String>,
    log: &Logger,
) -> FileResults {
    let cache_path = p.out_full.join(BUILD_CACHE_NAME);
    let manifest = manifest_digest(&p.repo_root_slash);
    let prev = if opts.full {
        None
    } else {
        load_build_cache(&cache_path)
    };
    let full = prev
        .as_ref()
        .is_none_or(|c| !same_strings(&c.files, files) || c.manifest != manifest);

    let ctx = ExtractContext::new(&p.repo_root_slash, files, known);
    let cands = mentions::candidates(files);
    let mut out = FileResults::default();
    let mut next = BuildCache {
        version: BUILD_CACHE_VERSION,
        files: files.to_vec(),
        manifest,
        entries: BTreeMap::new(),
    };

    for rel in files {
        let abs = join_root(&p.repo_root_slash, rel);
        let (size, mtime_ns) = stat(&abs);
        let old = if full {
            None
        } else {
            prev.as_ref().and_then(|c| c.entries.get(rel))
        };

        let entry = match old {
            // Unchanged size and mtime: the cached results stand, and the file is not read.
            Some(o) if o.size == size && o.mtime_ns == mtime_ns => o.clone(),
            _ => {
                let content = read_all(&abs);
                let hash = hash_content(&content);
                match old {
                    // Touched but not edited: the same content gives the same results.
                    Some(o) if o.hash == hash => CachedFile {
                        size,
                        mtime_ns,
                        ..o.clone()
                    },
                    _ => {
                        out.recomputed += 1;
                        compute(rel, &content, size, mtime_ns, hash, &ctx, &cands)
                    }
                }
            }
        };

        out.edges.extend(entry.edges.iter().cloned());
        if !entry.symbols.is_empty() {
            out.symbols.insert(rel.clone(), entry.symbols.clone());
        }
        next.entries.insert(rel.clone(), entry);
    }

    // The cache only saves work, so a failed write is reported and the build goes on.
    if let Err(err) = save_build_cache(&cache_path, &next) {
        log.warning(&format!(
            "could not save build cache (next build will be full): {err}"
        ));
    }
    out
}

fn compute(
    rel: &str,
    content: &str,
    size: u64,
    mtime_ns: i64,
    hash: String,
    ctx: &ExtractContext,
    cands: &[mentions::Candidate],
) -> CachedFile {
    if content.is_empty() {
        return CachedFile {
            size,
            mtime_ns,
            hash,
            symbols: Vec::new(),
            edges: Vec::new(),
        };
    }
    let mut edges = ctx.file_edges(rel, content);
    edges.extend(mentions::mention_edges_for(rel, content, cands));
    CachedFile {
        size,
        mtime_ns,
        hash,
        symbols: symbols_of(rel, content),
        edges,
    }
}

fn stat(abs: &Path) -> (u64, i64) {
    let Ok(meta) = fs::metadata(abs) else {
        return (0, 0);
    };
    let mtime_ns = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_nanos()).unwrap_or(i64::MAX));
    (meta.len(), mtime_ns)
}

#[must_use]
pub fn hash_content(content: &str) -> String {
    hex(&Sha256::digest(content.as_bytes()))
}

/// Digest of the `go.mod` and `pubspec.yaml` files, which the import resolvers read.
#[must_use]
pub fn manifest_digest(repo_root_slash: &str) -> String {
    let mut h = Sha256::new();
    for name in ["go.mod", "pubspec.yaml"] {
        for rel in find_files_named(repo_root_slash, name) {
            h.update(rel.as_bytes());
            h.update([0]);
            h.update(read_all(&join_root(repo_root_slash, &rel)).as_bytes());
            h.update([0]);
        }
    }
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The cache at `path`, or `None` when it is missing, unreadable or from another version.
#[must_use]
pub fn load_build_cache(path: &Path) -> Option<BuildCache> {
    let text = fs::read_to_string(path).ok()?;
    let cache: BuildCache = serde_json::from_str(&text).ok()?;
    (cache.version == BUILD_CACHE_VERSION).then_some(cache)
}

/// Writes the cache through a temporary file, so an interrupted write leaves no partial cache.
///
/// # Errors
///
/// The cache cannot be serialised or written.
pub fn save_build_cache(path: &Path, cache: &BuildCache) -> io::Result<()> {
    let json = serde_json::to_string(cache).map_err(io::Error::other)?;
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

#[must_use]
pub fn same_strings(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut sa: Vec<&String> = a.iter().collect();
    let mut sb: Vec<&String> = b.iter().collect();
    sa.sort();
    sb.sort();
    sa == sb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_strings_ignores_order() {
        let a = vec!["x".to_owned(), "y".to_owned()];
        let b = vec!["y".to_owned(), "x".to_owned()];
        assert!(same_strings(&a, &b));
        assert!(!same_strings(&a, &["x".to_owned()]));
    }

    #[test]
    fn a_stale_version_is_discarded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(BUILD_CACHE_NAME);
        fs::write(
            &path,
            r#"{"version":0,"files":[],"manifest":"","entries":{}}"#,
        )
        .expect("write");
        assert!(load_build_cache(&path).is_none());
    }
}
