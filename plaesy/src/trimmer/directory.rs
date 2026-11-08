//! Layer 2 over a path: one file, the Markdown files of a directory, or a whole tree.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::layer2::{compress_file, FileResult};
use super::rules::Rules;

/// Compresses `path` (relative to the working directory, or else to `repo_root`).
///
/// A directory compresses its own Markdown files, or every Markdown file below it when
/// `recurse` is set. Files are handled in name order, and the first failure stops the run.
///
/// # Errors
///
/// The path does not exist, a directory cannot be read, or a file fails to compress.
#[allow(clippy::too_many_arguments)]
pub fn compress_path(
    rules: &Rules,
    repo_root: &Path,
    graph_path: &Path,
    stats_path: Option<&Path>,
    path: &str,
    level: &str,
    recurse: bool,
    dry_run: bool,
) -> io::Result<Vec<FileResult>> {
    let mut target = PathBuf::from(path);
    if !target.exists() {
        target = repo_root.join(path);
    }
    let meta = fs::metadata(&target)
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, format!("path not found: {path}")))?;

    let mut files = Vec::new();
    if meta.is_dir() {
        if recurse {
            walk_markdown(&target, &mut files)?;
        } else {
            for entry in sorted_entries(&target)? {
                if !entry.file_type()?.is_dir() && is_markdown(&entry.path()) {
                    files.push(entry.path());
                }
            }
        }
    } else {
        files.push(target);
    }

    files
        .iter()
        .map(|f| compress_file(rules, repo_root, graph_path, stats_path, f, level, dry_run))
        .collect()
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

fn sorted_entries(dir: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(dir)?.collect::<io::Result<_>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn walk_markdown(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in sorted_entries(dir)? {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            walk_markdown(&path, files)?;
        } else if is_markdown(&path) {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_without_recursion_skips_subfolders() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("top.md"), "x").expect("write");
        fs::create_dir(dir.path().join("sub")).expect("mkdir");
        fs::write(dir.path().join("sub").join("deep.md"), "y").expect("write");
        let mut files = Vec::new();
        for entry in sorted_entries(dir.path()).expect("list") {
            if !entry.file_type().expect("type").is_dir() && is_markdown(&entry.path()) {
                files.push(entry.path());
            }
        }
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn recursion_finds_nested_markdown() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("sub")).expect("mkdir");
        fs::write(dir.path().join("sub").join("deep.MD"), "y").expect("write");
        let mut files = Vec::new();
        walk_markdown(dir.path(), &mut files).expect("walk");
        assert_eq!(files.len(), 1);
    }
}
