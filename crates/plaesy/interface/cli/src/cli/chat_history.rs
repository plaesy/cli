//! Saved chat sessions under `.plaesy/chats/`, one JSON-lines file per session.
//!
//! Each line is one turn: `{"role":"user"|"assistant","text":"..."}`. A session is written when it
//! ends with at least one turn, so a session that never sent a message leaves no file. `--resume
//! <name>` starts from a saved file, and `/find <text>` searches every saved file.
//!
//! The files are local. Nothing here is sent anywhere.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Most matches `/find` prints.
pub const MAX_HITS: usize = 20;

/// Folder under the repository where chat sessions are saved.
#[must_use]
pub fn dir(repo: &Path) -> PathBuf {
    repo.join(".plaesy").join("chats")
}

/// A new file name for this session, based on the time it ends.
#[must_use]
pub fn new_name() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!("chat-{millis}.jsonl")
}

/// Writes the turns to `dir/name` and returns the path. Creates the folder when needed.
///
/// # Errors
///
/// The I/O error from creating the folder or writing the file.
pub fn save(dir: &Path, name: &str, turns: &[(String, String)]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(name);
    let mut file = fs::File::create(&path)?;
    for (role, text) in turns {
        let line = json!({ "role": role, "text": text });
        writeln!(file, "{line}")?;
    }
    Ok(path)
}

/// Reads the turns of a saved file, oldest first. Lines that are not turns are skipped.
///
/// # Errors
///
/// The I/O error from opening or reading the file.
pub fn load(path: &Path) -> io::Result<Vec<(String, String)>> {
    let reader = BufReader::new(fs::File::open(path)?);
    let mut turns = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let role = value.get("role").and_then(Value::as_str);
        let text = value.get("text").and_then(Value::as_str);
        if let (Some(role @ ("user" | "assistant")), Some(text)) = (role, text) {
            turns.push((role.to_owned(), text.to_owned()));
        }
    }
    Ok(turns)
}

/// One line that matched a `/find` search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// File name under the chats folder.
    pub file: String,
    /// 1-based line number in that file.
    pub line: usize,
    /// The turn's text, cut to 120 characters.
    pub snippet: String,
}

/// Case-insensitive search of every saved turn. Returns at most [`MAX_HITS`] matches, newest file
/// first.
#[must_use]
pub fn search(dir: &Path, query: &str) -> Vec<Hit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    files.reverse();
    let mut hits = Vec::new();
    for path in files {
        let Ok(turns) = load(&path) else {
            continue;
        };
        let file = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        for (index, (_, text)) in turns.iter().enumerate() {
            if text.to_lowercase().contains(&needle) {
                hits.push(Hit {
                    file: file.clone(),
                    line: index + 1,
                    snippet: text.chars().take(120).collect(),
                });
                if hits.len() == MAX_HITS {
                    return hits;
                }
            }
        }
    }
    hits
}

#[cfg(test)]
#[path = "chat_history_test.rs"]
mod chat_history_test;

/// One saved session, for `/resume` listings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// File name under the chats folder.
    pub name: String,
    /// Number of turns in the file.
    pub turns: usize,
    /// The first user line, cut to 60 characters.
    pub first: String,
}

/// Every saved session, newest first. Files that cannot be read are left out.
#[must_use]
pub fn list(dir: &Path) -> Vec<Saved> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".jsonl"))
        .collect();
    names.sort();
    names.reverse();
    names
        .into_iter()
        .filter_map(|name| {
            let turns = load(&dir.join(&name)).ok()?;
            let first = turns
                .iter()
                .find(|(role, _)| role == "user")
                .map_or_else(String::new, |(_, text)| text.chars().take(60).collect());
            Some(Saved {
                name,
                turns: turns.len(),
                first,
            })
        })
        .collect()
}

/// A saved file name: a plain `.jsonl` name under the chats folder, with no path parts.
///
/// # Errors
///
/// A message when the name has a path separator, `..`, or lacks the `.jsonl` extension.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.contains(['/', '\\'])
        || name.contains("..")
        || !name.ends_with(".jsonl")
    {
        return Err(format!("nama sesi tidak valid: {name}"));
    }
    Ok(())
}

/// Deletes one saved session.
///
/// # Errors
///
/// An invalid name, or the I/O error when the file cannot be removed.
pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    check_name(name)?;
    fs::remove_file(dir.join(name)).map_err(|e| format!("{name}: {e}"))
}

/// Deletes every saved session and returns how many were removed.
///
/// # Errors
///
/// The I/O error from listing the folder or removing a file.
pub fn clear(dir: &Path) -> Result<usize, String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut removed = 0;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".jsonl") {
            fs::remove_file(entry.path()).map_err(|e| format!("{name}: {e}"))?;
            removed += 1;
        }
    }
    Ok(removed)
}
