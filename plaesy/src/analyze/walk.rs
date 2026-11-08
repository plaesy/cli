//! The one tree walk every generator shares. It excludes dotfile directories, `node_modules`,
//! and anything the project's `.gitignore` excludes.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::exclude::Ignore;

/// One file the walk kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// Project-relative, forward-slash separated.
    pub rel_path: String,
    pub size: u64,
    /// Modification time in whole seconds since the Unix epoch.
    pub mod_unix: i64,
}

/// One directory the walk kept, with its direct file count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirInfo {
    pub rel_path: String,
    pub file_count: usize,
}

/// Everything the walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WalkResult {
    pub files: Vec<FileInfo>,
    pub dirs: Vec<DirInfo>,
    pub newest_unix: i64,
}

/// Walks `project` once.
///
/// # Errors
///
/// The project path does not exist. Entries that become unreadable during the walk are skipped.
pub fn walk_project(project: &Path) -> io::Result<WalkResult> {
    fs::metadata(project)?;
    let ignore = Ignore::load(project);
    let mut res = WalkResult::default();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    walk_dir(project, "", &ignore, &mut res, &mut counts);
    for d in &mut res.dirs {
        d.file_count = counts.get(&d.rel_path).copied().unwrap_or(0);
    }
    Ok(res)
}

fn walk_dir(
    root: &Path,
    rel_dir: &str,
    ignore: &Ignore,
    res: &mut WalkResult,
    counts: &mut BTreeMap<String, usize>,
) {
    let dir = if rel_dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel_dir)
    };
    let Ok(read) = fs::read_dir(&dir) else {
        return;
    };
    let mut entries: Vec<fs::DirEntry> = read.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if rel_dir.is_empty() {
            name.clone()
        } else {
            format!("{rel_dir}/{name}")
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if ignore.skip_dir(&rel, &name) {
                continue;
            }
            res.dirs.push(DirInfo {
                rel_path: rel.clone(),
                file_count: 0,
            });
            walk_dir(root, &rel, ignore, res, counts);
        } else {
            if ignore.skip_file(&rel) {
                continue;
            }
            let (size, mod_unix) = entry
                .metadata()
                .map(|m| (m.len(), mtime_secs(&m)))
                .unwrap_or((0, 0));
            res.newest_unix = res.newest_unix.max(mod_unix);
            *counts.entry(rel_dir.to_owned()).or_insert(0) += 1;
            res.files.push(FileInfo {
                rel_path: rel,
                size,
                mod_unix,
            });
        }
    }
}

/// Whole seconds since the Unix epoch for a file's modification time, or 0 when unknown.
#[must_use]
pub fn mtime_secs(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(0)
}
