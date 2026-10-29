//! File discovery and lexical path resolution, with no filesystem resolution of links.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::exclude::Ignore;

use super::extensions::is_collected;

/// Forward-slash form of a path. Every backslash is replaced, whatever the OS, so node
/// names are the same string on every platform.
#[must_use]
pub fn to_slash(p: &str) -> String {
    p.replace('\\', "/")
}

/// Collects every collected file under `repo_root` as a sorted relative slash path.
/// Excluded directories and the output folder are pruned on the directory branch.
///
/// # Errors
///
/// I/O failures walking the root.
pub fn collect_files(repo_root: &Path, out_dir_norm: &str) -> io::Result<Vec<String>> {
    let ignore = Ignore::load(repo_root);
    let mut out = Vec::new();
    walk(repo_root, repo_root, &ignore, out_dir_norm, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(
    root: &Path,
    dir: &Path,
    ignore: &Ignore,
    out_dir: &str,
    out: &mut Vec<String>,
) -> io::Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        // Best-effort scan: an unreadable directory is skipped, not fatal.
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = to_slash(&path.strip_prefix(root).unwrap_or(&path).to_string_lossy());
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if ignore.skip_dir(&rel, &name) {
                continue;
            }
            if !out_dir.is_empty() && (rel == out_dir || rel.starts_with(&format!("{out_dir}/"))) {
                continue;
            }
            walk(root, &path, ignore, out_dir, out)?;
        } else if is_collected(&rel) && !ignore.skip_file(&rel) {
            out.push(rel);
        }
    }
    Ok(())
}

/// Lexical `realpath -m`: resolves `.` and `..` without touching the filesystem.
///
/// `base` is an absolute forward-slash directory. `p` may itself be absolute, in which case
/// `base` is ignored. `..` stops at the root, and a Windows drive letter stays attached.
#[must_use]
pub fn normalize_path(base: &str, p: &str) -> String {
    let combined = if p.starts_with('/') || is_windows_abs(p) || base.is_empty() {
        p.to_owned()
    } else if p.is_empty() {
        base.to_owned()
    } else {
        format!("{base}/{p}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in combined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| !is_drive_seg(last)) {
                    parts.pop();
                }
            }
            other => parts.push(other),
        }
    }
    let mut root = "/".to_owned();
    if parts.first().is_some_and(|first| is_drive_seg(first)) {
        root = format!("{}:/", &parts[0][..1]);
        parts.remove(0);
    }
    if parts.is_empty() {
        root
    } else {
        format!("{root}{}", parts.join("/"))
    }
}

fn is_drive_seg(seg: &str) -> bool {
    let bytes = seg.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn is_windows_abs(p: &str) -> bool {
    let bytes = p.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// Resolves a link inside `from_rel` to a known repo-relative node id, or `None`.
/// External links, anchors, empty targets, and the root itself are not nodes.
#[must_use]
pub fn resolve_rel_link(
    repo_root_slash: &str,
    from_rel: &str,
    link: &str,
    known: &std::collections::BTreeSet<String>,
) -> Option<String> {
    if link.starts_with("http://") || link.starts_with("https://") || link.starts_with('#') {
        return None;
    }
    let target = link.split('#').next().unwrap_or("");
    if target.is_empty() {
        return None;
    }
    let from_dir = match from_rel.rfind('/') {
        Some(i) => format!("{repo_root_slash}/{}", &from_rel[..i]),
        None => repo_root_slash.to_owned(),
    };
    let resolved = normalize_path(&from_dir, target);
    let prefix = format!("{repo_root_slash}/");
    if resolved == repo_root_slash || !format!("{resolved}/").starts_with(&prefix) {
        return None;
    }
    let rel = &resolved[prefix.len()..];
    known.contains(rel).then(|| rel.to_owned())
}

/// Reads a file's text, or the empty string when it cannot be read.
#[must_use]
pub fn read_all(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Joins a root and a relative slash path into an OS path.
#[must_use]
pub fn join_root(repo_root_slash: &str, rel: &str) -> PathBuf {
    PathBuf::from(format!("{repo_root_slash}/{rel}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_keeps_the_root_and_drive_letters() {
        assert_eq!(
            normalize_path("/repo/docs", "../src/a.md"),
            "/repo/src/a.md"
        );
        assert_eq!(normalize_path("/repo", "../../../x"), "/x");
        assert_eq!(normalize_path("C:/repo", "../x"), "C:/x");
        assert_eq!(normalize_path("/repo", "/abs/y"), "/abs/y");
    }

    #[test]
    fn links_resolve_only_to_known_nodes() {
        let known: std::collections::BTreeSet<String> = ["docs/b.md".to_owned()].into();
        assert_eq!(
            resolve_rel_link("/repo", "docs/a.md", "b.md#sec", &known),
            Some("docs/b.md".into())
        );
        assert_eq!(
            resolve_rel_link("/repo", "docs/a.md", "https://x", &known),
            None
        );
        assert_eq!(
            resolve_rel_link("/repo", "docs/a.md", "missing.md", &known),
            None
        );
    }
}
