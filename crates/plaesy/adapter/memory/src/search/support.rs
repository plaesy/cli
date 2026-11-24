//! Free helpers for indexing, fingerprints and temp-file reconciliation.

use super::*;

pub(super) fn git_revision(dir: &Path) -> Option<String> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Hash one vault-relative note without opening the embedding or search engines.
pub fn fingerprint_note_in_root(root: &Path, relative_path: &Path) -> Result<String> {
    use std::path::Component;

    if relative_path.as_os_str().is_empty()
        || relative_path.is_absolute()
        || relative_path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(MemoryError::ConfigError(
            "fingerprint requires a relative file path inside the vault".into(),
        ));
    }
    let real_root = std::fs::canonicalize(root).map_err(|source| MemoryError::IoError {
        path: root.display().to_string(),
        source,
    })?;
    let source = real_root.join(relative_path);
    let meta = std::fs::symlink_metadata(&source).map_err(|source_error| MemoryError::IoError {
        path: source.display().to_string(),
        source: source_error,
    })?;
    if !meta.file_type().is_file() {
        return Err(MemoryError::ConfigError(format!(
            "fingerprint refuses a non-regular file: {}",
            source.display()
        )));
    }
    let real_source =
        std::fs::canonicalize(&source).map_err(|source_error| MemoryError::IoError {
            path: source.display().to_string(),
            source: source_error,
        })?;
    if !real_source.starts_with(&real_root) {
        return Err(MemoryError::ConfigError(format!(
            "fingerprint path resolves outside the vault: {}",
            source.display()
        )));
    }
    let content =
        std::fs::read_to_string(&real_source).map_err(|source_error| MemoryError::IoError {
            path: real_source.display().to_string(),
            source: source_error,
        })?;
    Ok(crate::memory::indexer::content_hash(&content))
}

pub(super) fn archived_note_content(content: &str, document: &Document) -> Result<String> {
    let line_ending = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let Some(frontmatter) = document.frontmatter_raw.as_deref() else {
        let (bom, body) = content
            .strip_prefix('\u{feff}')
            .map(|body| ("\u{feff}", body))
            .unwrap_or(("", content));
        return Ok(format!(
            "{bom}---{line_ending}status: archived{line_ending}---{line_ending}{body}"
        ));
    };
    let bom = if content.starts_with('\u{feff}') {
        "\u{feff}"
    } else {
        ""
    };

    let mut lines: Vec<String> = frontmatter.split('\n').map(str::to_owned).collect();
    if lines.first().map(String::as_str) != Some("---")
        || !matches!(lines.last().map(String::as_str), Some("---" | "..."))
    {
        return Err(MemoryError::ConfigError(
            "cannot archive because front matter boundaries are invalid".into(),
        ));
    }
    let closing = lines.len() - 1;
    let status_lines: Vec<usize> = (1..closing)
        .filter(|index| {
            let line = &lines[*index];
            if line.starts_with(char::is_whitespace) {
                return false;
            }
            line.split_once(':')
                .is_some_and(|(key, _)| key.trim().trim_matches(['"', '\'']) == "status")
        })
        .collect();
    if status_lines.len() > 1 {
        return Err(MemoryError::ConfigError(
            "cannot archive because front matter has duplicate status fields".into(),
        ));
    }
    if let Some(index) = status_lines.first().copied() {
        let comment = yaml_comment_suffix(&lines[index]);
        lines[index] = if let Some(comment) = comment {
            format!("status: archived {comment}")
        } else {
            "status: archived".into()
        };
    } else {
        lines.insert(closing, "status: archived".into());
    }
    let normalized_frontmatter = lines.join(line_ending);
    let body = document.body.replace('\n', line_ending);
    Ok(format!("{bom}{normalized_frontmatter}{line_ending}{body}"))
}

fn yaml_comment_suffix(line: &str) -> Option<&str> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote == Some('"') {
            escaped = true;
            continue;
        }
        if matches!(character, '"' | '\'') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
            continue;
        }
        if character == '#'
            && quote.is_none()
            && (index == 0 || line[..index].ends_with(char::is_whitespace))
        {
            return Some(&line[index..]);
        }
    }
    None
}

/// Crash/power-loss recovery (startup reconciliation): remove leftover note-publication
/// temporary files left behind by a process that died between creating `.{slug}.tmp-{pid}-{nonce}`
/// and either linking it into place or deleting it (see `SearchEngine::remember`).
///
/// These files are never themselves indexed (the dot-prefix hides them and they are not
/// `.md`/`.txt`), so without this they would accumulate forever after any crash, abrupt power
/// loss, or `kill -9` mid-write. Only files whose modification time is older than a safety
/// margin are removed, so an in-flight write on another thread/process is never touched: this
/// function is called while holding `WriteLock`, which already serializes it against new
/// `remember` calls, but an existing writer's open file handle predates that lock acquisition by
/// definition and must not be deleted out from under it.
pub(super) fn reconcile_stale_temp_files(root: &Path) {
    const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(60);
    let now = std::time::SystemTime::now();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !(e.file_type().is_dir()
                    && is_skipped_dir_name(&e.file_name().to_string_lossy()))
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if !(name.starts_with('.') && name.contains(".tmp-")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let age = now.duration_since(modified).unwrap_or_default();
        if age < STALE_AFTER {
            continue;
        }
        if let Err(e) = std::fs::remove_file(entry.path()) {
            tracing::warn!(
                "could not remove stale temp file {}: {e}",
                entry.path().display()
            );
        } else {
            tracing::info!(
                "removed stale temp file left by an interrupted write: {}",
                entry.path().display()
            );
        }
    }
}
