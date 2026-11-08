//! Removal of orphaned files from the tool-owned mirror trees.
//!
//! `reload` copies but never deletes. Files in the user's own `.plaesy/` tree
//! may be hand-written, so they are never pruned. The agent roles mirror and the
//! platform prompt mirror are owned by the tool. A file there with no source is a
//! stale copy that still runs as a command, so it can be pruned. Pruning reports
//! by default and deletes only with `--apply`, because these trees are usually
//! gitignored.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::log::Logger;
use crate::config::platform::PlatformConfig;

use super::dir_has_files;
use super::platform::{agent_extension, prompt_extension};

/// Prompt destinations shared with other content. A hand-written file there cannot
/// be told apart from a stale mirror, so these are never pruned.
const SHARED_PROMPT_DIRS: [&str; 2] = [".cursor/rules", ".qoder/rules"];

/// A destination tree the tool owns outright.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PruneRoot {
    pub label: &'static str,
    /// Absolute destination root.
    pub dest_dir: PathBuf,
    /// Absolute source root the mirror was copied from.
    pub src_dir: PathBuf,
    /// Source suffix replaced by `dst_ext`.
    pub src_strip: &'static str,
    /// Extension of the copied file.
    pub dst_ext: &'static str,
}

/// Owned trees for this run. The prompt mirror is pruned only when the platform
/// marks it `prune_prompts`, and no platform means no prompt mirror.
#[must_use]
pub fn prune_roots(
    home: &Path,
    abs_target: &Path,
    base_dir: &Path,
    platform: &str,
    cfg: &PlatformConfig,
) -> Vec<PruneRoot> {
    let agents_src = home.join("agents");
    let canonical_roles = base_dir.join("roles");
    let roles_dest = cfg
        .platforms
        .get(platform)
        .and_then(|entry| entry.mapping.get("agents"))
        .filter(|dest| !dest.is_empty())
        .map_or_else(|| canonical_roles.clone(), |dest| abs_target.join(dest));

    let mut roots = vec![PruneRoot {
        label: "agent roles",
        dest_dir: roles_dest.clone(),
        src_dir: agents_src.clone(),
        src_strip: ".agents.md",
        dst_ext: agent_extension(platform),
    }];
    if canonical_roles != roles_dest {
        roots.push(PruneRoot {
            label: "role mirror",
            dest_dir: canonical_roles,
            src_dir: agents_src,
            src_strip: ".agents.md",
            dst_ext: ".md",
        });
    }

    if platform.is_empty() {
        return roots;
    }
    let Some(entry) = cfg.platforms.get(platform) else {
        return roots;
    };
    let Some(dest_rel) = entry.mapping.get("prompts").filter(|d| !d.is_empty()) else {
        return roots;
    };
    if SHARED_PROMPT_DIRS.contains(&dest_rel.as_str()) {
        return roots;
    }
    let allowed = entry
        .mapping
        .get("prune_prompts")
        .is_some_and(|flag| parse_bool(flag));
    if !allowed {
        return roots;
    }
    let Some(prompt_source) = cfg
        .plaesy
        .mapping
        .get("prompts")
        .filter(|m| !m.value.is_empty())
    else {
        return roots;
    };

    roots.push(PruneRoot {
        label: "prompt mirror",
        dest_dir: abs_target.join(dest_rel),
        src_dir: home.join(prompt_source.value.trim_end_matches("/*")),
        src_strip: ".md",
        dst_ext: prompt_extension(platform),
    });
    roots
}

/// Accepts the same spellings as Go's `strconv.ParseBool`.
fn parse_bool(value: &str) -> bool {
    matches!(value, "1" | "t" | "T" | "TRUE" | "true" | "True")
}

/// Destination-relative paths the mirror would contain for the current sources.
///
/// # Errors
///
/// I/O errors walking the source. Callers must not prune when this fails: an
/// empty expected set would make every destination file look like an orphan.
pub fn expected_paths(root: &PruneRoot) -> std::io::Result<HashSet<PathBuf>> {
    let mut expected = HashSet::new();
    let mut sources = Vec::new();
    walk_files(&root.src_dir, &mut sources)?;
    for source in sources {
        let Some(name) = source.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(root.src_strip) else {
            continue;
        };
        let rel = source.strip_prefix(&root.src_dir).unwrap_or(&source);
        let parent = rel.parent().unwrap_or(Path::new(""));
        expected.insert(parent.join(format!("{stem}{}", root.dst_ext)));
    }
    Ok(expected)
}

/// Files that the owned trees contain but the sources no longer produce, as
/// slash-separated paths relative to the project. Roots whose source is missing,
/// empty, or unreadable are skipped with a warning.
#[must_use]
pub fn find_candidates(roots: &[PruneRoot], abs_target: &Path, log: &Logger) -> Vec<String> {
    let mut found = Vec::new();
    for root in roots {
        if !root.src_dir.is_dir() {
            log.warning(&format!(
                "Skipping {} prune — source not found: {}",
                root.label,
                root.src_dir.display()
            ));
            continue;
        }
        // An empty source gives an empty expected set, and that would mark every
        // file as an orphan.
        if !dir_has_files(&root.src_dir) {
            log.warning(&format!(
                "Skipping {} prune — source is empty: {}",
                root.label,
                root.src_dir.display()
            ));
            continue;
        }
        let expected = match expected_paths(root) {
            Ok(expected) => expected,
            Err(e) => {
                log.warning(&format!(
                    "Skipping {} prune — source unreadable: {e}",
                    root.label
                ));
                continue;
            }
        };
        if !root.dest_dir.exists() {
            continue;
        }
        let mut files = Vec::new();
        let _ = walk_files(&root.dest_dir, &mut files);
        for file in files {
            let Ok(rel) = file.strip_prefix(&root.dest_dir) else {
                continue;
            };
            if expected.contains(rel) {
                continue;
            }
            if let Ok(shown) = file.strip_prefix(abs_target) {
                found.push(shown.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found
}

/// Deletes the candidates that sit inside an owned tree, then removes directories
/// that the deletions leave empty. Returns the paths actually removed.
pub fn apply(
    roots: &[PruneRoot],
    abs_target: &Path,
    candidates: &[String],
    log: &Logger,
) -> Vec<String> {
    let mut removed = Vec::new();
    for rel in candidates {
        let path = abs_target.join(rel);
        if !roots.iter().any(|root| path.starts_with(&root.dest_dir)) {
            log.warning(&format!(
                "Refusing to remove {rel} — outside every owned tree"
            ));
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => removed.push(rel.clone()),
            Err(e) => log.warning(&format!("  ✗ {rel} ({e})")),
        }
    }
    for root in roots {
        remove_empty_dirs(&root.dest_dir);
    }
    removed
}

/// Removes directories under `root` that hold no files, deepest first. `root` itself stays.
fn remove_empty_dirs(root: &Path) {
    let mut dirs = Vec::new();
    collect_dirs(root, &mut dirs);
    for dir in dirs.iter().rev() {
        let empty = fs::read_dir(dir)
            .map(|mut e| e.next().is_none())
            .unwrap_or(false);
        if empty {
            let _ = fs::remove_dir(dir);
        }
    }
}

fn collect_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    children.sort();
    for child in children {
        out.push(child.clone());
        collect_dirs(&child, out);
    }
}

/// Every regular file below `dir`, sorted.
fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(src: &Path, dest: &Path) -> PruneRoot {
        PruneRoot {
            label: "test",
            dest_dir: dest.to_path_buf(),
            src_dir: src.to_path_buf(),
            src_strip: ".agents.md",
            dst_ext: ".md",
        }
    }

    #[test]
    fn orphan_in_the_owned_tree_is_a_candidate_and_apply_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dest = dir.path().join(".plaesy/roles");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(dest.join("old")).unwrap();
        fs::write(src.join("kept.agents.md"), "x").unwrap();
        fs::write(dest.join("kept.md"), "x").unwrap();
        fs::write(dest.join("old/gone.md"), "stale").unwrap();

        let roots = vec![root(&src, &dest)];
        let log = Logger::new("t");
        let candidates = find_candidates(&roots, dir.path(), &log);
        assert_eq!(candidates, [".plaesy/roles/old/gone.md"]);

        let removed = apply(&roots, dir.path(), &candidates, &log);
        assert_eq!(removed, candidates);
        assert!(!dest.join("old").exists(), "empty directory is swept");
        assert!(dest.join("kept.md").exists());
    }

    #[test]
    fn empty_source_never_prunes_anything() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dest = dir.path().join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("precious.md"), "mine").unwrap();
        let candidates = find_candidates(&[root(&src, &dest)], dir.path(), &Logger::new("t"));
        assert!(candidates.is_empty());
    }

    #[test]
    fn bool_spellings_match_go_parse_bool() {
        for yes in ["1", "t", "true", "TRUE", "True"] {
            assert!(parse_bool(yes), "{yes}");
        }
        for no in ["0", "f", "false", "no", ""] {
            assert!(!parse_bool(no), "{no}");
        }
    }
}
