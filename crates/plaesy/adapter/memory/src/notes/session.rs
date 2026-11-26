//! Session summaries saved as memory notes: a title, the session date, a summary and links.
//!
//! A summary is an ordinary note written through [`SearchEngine::remember`], so it is indexed at
//! once and found by the existing keyword and semantic search. It adds no storage path and no MCP
//! tool: `memory_remember` already accepts the same fields.

use super::NewNote;
use crate::memory::error::{MemoryError, Result};
use crate::memory::search::SearchEngine;
use chrono::NaiveDate;
use std::path::PathBuf;

/// Folder under the notes root where session summaries are written.
pub const SESSION_FOLDER: &str = "sessions";

/// Tier of a session summary: it records what happened on one day.
pub const SESSION_TIER: &str = "episodic";

/// Most links one session summary may carry.
pub const MAX_SESSION_LINKS: usize = 20;

/// A link listed under a session summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionLink {
    /// Text shown for the link. Must not contain brackets or control characters.
    pub label: String,
    /// `http://` or `https://` address without whitespace or Markdown-breaking characters.
    pub url: String,
}

/// A summary of one working session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSummary {
    /// Session title, for example the task or goal of the session.
    pub title: String,
    /// Date the session took place.
    pub date: NaiveDate,
    /// Markdown summary of what was done, decided and left open.
    pub summary: String,
    /// Related links: pull requests, commits, documents, and similar.
    pub links: Vec<SessionLink>,
}

impl SessionSummary {
    /// Build the note this summary is saved as.
    ///
    /// The title becomes `Sesi <date>: <title>`, the body starts with the session date, then the
    /// summary, then a `Tautan` list when there are links.
    ///
    /// # Errors
    ///
    /// - `ConfigError` when the title or summary is empty, the title has control characters,
    ///   there are more than [`MAX_SESSION_LINKS`] links, or a link has an empty label, a label
    ///   with brackets or control characters, or a URL that is not plain `http(s)`
    pub fn to_note(&self) -> Result<NewNote> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(MemoryError::ConfigError(
                "session title must not be empty".into(),
            ));
        }
        if title.chars().any(char::is_control) {
            return Err(MemoryError::ConfigError(
                "session title must not contain control characters".into(),
            ));
        }
        let summary = self.summary.trim();
        if summary.is_empty() {
            return Err(MemoryError::ConfigError(
                "session summary must not be empty".into(),
            ));
        }
        if self.links.len() > MAX_SESSION_LINKS {
            return Err(MemoryError::ConfigError(format!(
                "a session summary takes at most {MAX_SESSION_LINKS} links"
            )));
        }
        let mut body = format!("Tanggal: {}\n\n{summary}\n", self.date.format("%Y-%m-%d"));
        if !self.links.is_empty() {
            body.push_str("\n## Tautan\n\n");
            for link in &self.links {
                validate_link(link)?;
                body.push_str(&format!("- [{}]({})\n", link.label.trim(), link.url));
            }
        }
        Ok(NewNote {
            folder: SESSION_FOLDER.into(),
            title: format!("Sesi {}: {title}", self.date.format("%Y-%m-%d")),
            body,
            tier: SESSION_TIER.into(),
            confidence: "medium".into(),
            graph: None,
        })
    }
}

fn validate_link(link: &SessionLink) -> Result<()> {
    let label = link.label.trim();
    if label.is_empty() {
        return Err(MemoryError::ConfigError(
            "session link label must not be empty".into(),
        ));
    }
    if label
        .chars()
        .any(|c| c.is_control() || c == '[' || c == ']')
    {
        return Err(MemoryError::ConfigError(format!(
            "session link label has brackets or control characters: {label}"
        )));
    }
    let url = link.url.as_str();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(MemoryError::ConfigError(format!(
            "session link must be an http or https address: {url}"
        )));
    }
    if url
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '(' | ')' | '<' | '>'))
    {
        return Err(MemoryError::ConfigError(format!(
            "session link address has whitespace or Markdown-breaking characters: {url}"
        )));
    }
    Ok(())
}

impl SearchEngine {
    /// Save a session summary under [`SESSION_FOLDER`] and index it, returning the note path.
    ///
    /// # Errors
    ///
    /// Same as [`SessionSummary::to_note`] and [`SearchEngine::remember`].
    pub async fn remember_session(&self, summary: &SessionSummary) -> Result<PathBuf> {
        let note = summary.to_note()?;
        self.remember(&note).await
    }
}

#[cfg(test)]
#[path = "session_test.rs"]
mod session_test;
