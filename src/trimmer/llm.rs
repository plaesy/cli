//! The LLM-quality path for Layer 2. The calling assistant rewrites the prose the queue
//! lists, and `apply` merges those rewrites back. No API key is needed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::layer2::{backup_path, is_fence_line, relative_to, split_keep_trailing};
use super::stats;

/// Prose segments shorter than this are not queued.
const MIN_SEGMENT_CHARS: usize = 40;

/// One span of a file: prose or code, split at fence lines. `index` is its position in the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    index: usize,
    text: String,
    was_prose: bool,
}

fn split_segments(text: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut buf = String::new();
    let mut in_code = false;
    let mut index = 0;
    for line in split_keep_trailing(text) {
        buf.push_str(line);
        buf.push('\n');
        if is_fence_line(line) {
            segments.push(Segment {
                index,
                text: std::mem::take(&mut buf),
                was_prose: !in_code,
            });
            in_code = !in_code;
            index += 1;
        }
    }
    segments.push(Segment {
        index,
        text: buf,
        was_prose: !in_code,
    });
    segments
}

/// One queued segment, as written to the queue file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueSegment {
    pub index: usize,
    pub text: String,
}

/// The queue file: the file it was made for and its prose segments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub segments: Vec<QueueSegment>,
}

/// Errors from the LLM path.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("path not found: {0}")]
    PathNotFound(String),
    #[error("annotations file not found: {0}")]
    AnnotationsNotFound(String),
    #[error("invalid annotations JSON: {0}")]
    InvalidAnnotations(String),
    #[error("annotations were produced for {queued:?} but --path resolves to {target:?}: the segment indexes belong to the queued file, not this one (re-run `plaesy trim llm-queue --path {target}`)")]
    WrongFile { queued: String, target: String },
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// Writes the prose segments of `path` (at least 40 characters) to `<repo>/.plaesy/analysis/trim-queue.json`.
/// Returns the queue path and the number of segments written.
///
/// # Errors
///
/// A missing file, or I/O failures writing the queue.
pub fn write_queue(repo_root: &Path, path: &Path) -> Result<(PathBuf, usize), LlmError> {
    let target = resolve_target(repo_root, path)?;
    let text = fs::read_to_string(&target)
        .map_err(|_| LlmError::PathNotFound(path.display().to_string()))?;
    let relative = relative_to(repo_root, &target);
    let segments: Vec<QueueSegment> = split_segments(&text)
        .into_iter()
        .filter(|s| s.was_prose && s.text.len() >= MIN_SEGMENT_CHARS)
        .map(|s| QueueSegment {
            index: s.index,
            text: s.text,
        })
        .collect();

    let out_dir = repo_root.join(".plaesy").join("analysis");
    fs::create_dir_all(&out_dir)?;
    let queue_path = out_dir.join("trim-queue.json");
    let queue = Queue {
        file: relative,
        segments: segments.clone(),
    };
    let mut json = serde_json::to_string_pretty(&queue).map_err(io::Error::other)?;
    json.push('\n');
    fs::write(&queue_path, json)?;
    Ok((queue_path, segments.len()))
}

/// Result of applying rewrites to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    pub relative: String,
    pub before: usize,
    pub after: usize,
}

/// Merges the rewrites in `annotations` into `path`. The original is kept as `<file>.bak`.
///
/// # Errors
///
/// A missing file or annotations file, invalid annotations, a queue made for another
/// file, or I/O failures.
pub fn apply(
    repo_root: &Path,
    stats_path: Option<&Path>,
    path: &Path,
    annotations: &Path,
) -> Result<ApplyResult, LlmError> {
    let target = resolve_target(repo_root, path)?;
    let original = fs::read_to_string(&target)
        .map_err(|_| LlmError::PathNotFound(path.display().to_string()))?;
    let raw = fs::read_to_string(annotations)
        .map_err(|_| LlmError::AnnotationsNotFound(annotations.display().to_string()))?;
    let queue: Queue =
        serde_json::from_str(&raw).map_err(|e| LlmError::InvalidAnnotations(e.to_string()))?;

    let relative = relative_to(repo_root, &target);
    if !queue.file.is_empty() && !same_file(&queue.file, &relative) {
        return Err(LlmError::WrongFile {
            queued: queue.file,
            target: relative,
        });
    }

    // The first rewrite for an index wins.
    let mut rewrites = std::collections::BTreeMap::new();
    for segment in &queue.segments {
        rewrites
            .entry(segment.index)
            .or_insert(segment.text.as_str());
    }

    let mut out = String::new();
    for segment in split_segments(&original) {
        match rewrites.get(&segment.index).filter(|_| segment.was_prose) {
            Some(rewrite) => out.push_str(rewrite),
            None => out.push_str(&segment.text),
        }
    }

    let before = stats::estimate_tokens(&original);
    let after = stats::estimate_tokens(&out);
    fs::write(backup_path(&target), &original)?;
    fs::write(&target, &out)?;
    if let Some(stats_path) = stats_path {
        stats::add_record(stats_path, "layer2-llm", &relative, before, after)?;
    }
    Ok(ApplyResult {
        relative,
        before,
        after,
    })
}

fn resolve_target(repo_root: &Path, path: &Path) -> Result<PathBuf, LlmError> {
    if path.exists() {
        return Ok(path.to_path_buf());
    }
    Ok(repo_root.join(path))
}

/// Compares two spellings of one path by their absolute form, falling back to the cleaned text.
fn same_file(a: &str, b: &str) -> bool {
    let norm = |p: &str| {
        let cleaned = Path::new(p).components().collect::<PathBuf>();
        std::path::absolute(&cleaned).unwrap_or(cleaned)
    };
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_alternate_prose_and_code_at_fences() {
        let segments = split_segments("intro line\n```\ncode\n```\nafter\n");
        let kinds: Vec<bool> = segments.iter().map(|s| s.was_prose).collect();
        assert_eq!(kinds, [true, false, true]);
        assert_eq!(segments[2].index, 2);
    }

    #[test]
    fn apply_rewrites_only_the_matching_prose_segment() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.md");
        fs::write(
            &file,
            "This is a long enough prose line for the queue.\n```\nkeep\n```\n",
        )
        .unwrap();
        let ann = dir.path().join("ann.json");
        fs::write(
            &ann,
            r#"{"file":"notes.md","segments":[{"index":0,"text":"Short rewrite.\n```\n"}]}"#,
        )
        .unwrap();
        let result = apply(dir.path(), None, &file, &ann).unwrap();
        assert_eq!(result.relative, "notes.md");
        let merged = fs::read_to_string(&file).unwrap();
        assert!(merged.starts_with("Short rewrite.\n```\nkeep"));
        assert!(backup_path(&file).exists());
    }

    #[test]
    fn a_queue_for_another_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.md");
        fs::write(&file, "text\n").unwrap();
        let ann = dir.path().join("ann.json");
        fs::write(&ann, r#"{"file":"other.md","segments":[]}"#).unwrap();
        assert!(matches!(
            apply(dir.path(), None, &file, &ann),
            Err(LlmError::WrongFile { .. })
        ));
    }
}
