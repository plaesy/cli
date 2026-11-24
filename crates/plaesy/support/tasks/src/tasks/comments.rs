//! Comments on a task, kept in `comments.jsonl` inside the task folder.
//!
//! Each line is one comment as a JSON object. An `@name` in the body is a mention. The names
//! are extracted when the comment is written and stored in `mentions`, so readers do not parse
//! the text again. A task's comments move with its folder.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use super::issue::IssueGraphLock;
use super::{find_task, TasksError};

/// File inside a task folder that holds its comments, one JSON object per line.
pub const COMMENTS_FILE: &str = "comments.jsonl";

/// One comment on a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Sequence number within the task, starting at 1. Used to remove a comment.
    pub id: u64,
    /// Comment text as written, trimmed at both ends.
    pub body: String,
    /// Names mentioned with `@` in `body`, distinct, in order of first appearance, without the `@`.
    pub mentions: Vec<String>,
    /// UTC timestamp in RFC 3339 form with second precision.
    pub created_at: String,
}

/// Names mentioned in `body` as `@name`: distinct, in order of first appearance, without the `@`.
///
/// A name is a run of ASCII letters, digits, `_`, `-`, or `.`. A trailing `.` is punctuation,
/// so `@ana.` mentions `ana`. An `@` right after a name character belongs to another token,
/// such as an e-mail address, and is not a mention.
#[must_use]
pub fn extract_mentions(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut mentions: Vec<String> = Vec::new();
    for (at, &byte) in bytes.iter().enumerate() {
        if byte != b'@' || (at > 0 && is_name_byte(bytes[at - 1])) {
            continue;
        }
        let start = at + 1;
        let end = bytes[start..]
            .iter()
            .position(|&b| !is_name_byte(b))
            .map_or(bytes.len(), |offset| start + offset);
        // Every byte in `start..end` is ASCII, so both ends sit on character boundaries.
        let name = body[start..end].trim_end_matches('.');
        if !name.bytes().any(|b| b.is_ascii_alphanumeric()) {
            continue;
        }
        if mentions.iter().any(|known| known == name) {
            continue;
        }
        mentions.push(name.to_owned());
    }
    mentions
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

/// Adds a comment to `task` and returns it as stored.
///
/// # Errors
///
/// [`TasksError::EmptyComment`] when the text is blank, [`TasksError::NotFound`] when the task
/// does not exist, and I/O or JSON errors while reading or appending the comments file.
pub fn add_comment(tasks_dir: &Path, task: &str, body: &str) -> Result<Comment, TasksError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(TasksError::EmptyComment);
    }
    // Held across read, id choice, and append, so two writers never share an ID.
    let _guard = lock_comments(tasks_dir)?;
    let path = comments_path(tasks_dir, task)?;
    let text = read_comments_text(&path)?;
    let existing = parse_comments(&path, &text)?;
    let comment = Comment {
        id: existing.iter().map(|c| c.id).max().map_or(1, |max| max + 1),
        body: body.to_owned(),
        mentions: extract_mentions(body),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
    };
    let mut appended = String::new();
    if !text.is_empty() && !text.ends_with('\n') {
        // A hand-edited file may lack a final newline; keep the new record on its own line.
        appended.push('\n');
    }
    appended.push_str(&encode_line(task, &comment)?);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| TasksError::io(format!("opening {}", path.display()), e))?;
    file.write_all(appended.as_bytes())
        .map_err(|e| TasksError::io(format!("writing {}", path.display()), e))?;
    Ok(comment)
}

/// Lists the comments of `task` in the order they were written.
///
/// # Errors
///
/// [`TasksError::NotFound`] when the task does not exist, and I/O or JSON errors while reading
/// the comments file.
pub fn list_comments(tasks_dir: &Path, task: &str) -> Result<Vec<Comment>, TasksError> {
    let path = comments_path(tasks_dir, task)?;
    let text = read_comments_text(&path)?;
    parse_comments(&path, &text)
}

/// Removes the comment with `id` from `task`.
///
/// # Errors
///
/// [`TasksError::NotFound`] when the task does not exist, [`TasksError::CommentNotFound`] when
/// no comment has that ID, and I/O or JSON errors while rewriting the comments file.
pub fn remove_comment(tasks_dir: &Path, task: &str, id: u64) -> Result<(), TasksError> {
    // Held until the rename, so a concurrent append cannot land between read and replace.
    let _guard = lock_comments(tasks_dir)?;
    let path = comments_path(tasks_dir, task)?;
    let text = read_comments_text(&path)?;
    let comments = parse_comments(&path, &text)?;
    if !comments.iter().any(|comment| comment.id == id) {
        return Err(TasksError::CommentNotFound {
            task: task.to_owned(),
            id,
        });
    }
    let mut kept = String::new();
    for comment in comments.iter().filter(|comment| comment.id != id) {
        kept.push_str(&encode_line(task, comment)?);
    }
    // Write beside the file and rename, so a failed write leaves the old comments in place.
    let tmp = path.with_file_name(format!("{COMMENTS_FILE}.tmp"));
    fs::write(&tmp, kept.as_bytes())
        .map_err(|e| TasksError::io(format!("writing {}", tmp.display()), e))?;
    fs::rename(&tmp, &path)
        .map_err(|e| TasksError::io(format!("replacing {}", path.display()), e))
}

/// Exclusive guard over the issue graph, shared with issue updates. It is taken on the
/// collection lock file, not on `comments.jsonl`, because a rename cannot replace a file
/// that another handle holds open on Windows.
fn lock_comments(tasks_dir: &Path) -> Result<IssueGraphLock, TasksError> {
    IssueGraphLock::acquire(tasks_dir)
        .map_err(|message| TasksError::io("locking comments", io::Error::other(message)))
}

/// `<task folder>/comments.jsonl`, after checking that the task exists.
fn comments_path(tasks_dir: &Path, task: &str) -> Result<PathBuf, TasksError> {
    let (_, dir) =
        find_task(tasks_dir, task).ok_or_else(|| TasksError::NotFound(task.to_owned()))?;
    Ok(dir.join(COMMENTS_FILE))
}

/// Text of the comments file. A missing file is an empty list of comments.
fn read_comments_text(path: &Path) -> Result<String, TasksError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(TasksError::io(
            format!("refusing symlinked comments file {}", path.display()),
            io::Error::new(io::ErrorKind::InvalidInput, "comments file is a symlink"),
        )),
        Ok(_) => fs::read_to_string(path)
            .map_err(|e| TasksError::io(format!("reading {}", path.display()), e)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(TasksError::io(format!("reading {}", path.display()), error)),
    }
}

/// Parses one comment per non-blank line. A bad line is an error that names the line.
fn parse_comments(path: &Path, text: &str) -> Result<Vec<Comment>, TasksError> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<Comment>(line).map_err(|source| TasksError::Json {
                context: format!("reading {} line {}", path.display(), index + 1),
                source,
            })
        })
        .collect()
}

/// One comment as a JSON line, with its trailing newline.
fn encode_line(task: &str, comment: &Comment) -> Result<String, TasksError> {
    let mut line = serde_json::to_string(comment).map_err(|source| TasksError::Json {
        context: format!("encoding comment for {task}"),
        source,
    })?;
    line.push('\n');
    Ok(line)
}

#[cfg(test)]
#[path = "comments_test.rs"]
mod comments_test;
