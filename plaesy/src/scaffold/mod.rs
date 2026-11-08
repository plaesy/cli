//! Creates and refreshes the `.plaesy/` project structure.
//!
//! Port of `scripts/internal/scaffold` (Go). The module is split as:
//!
//! - [`home`]: locating the Plaesy source home
//! - [`config`]: `platform.json` with the scaffold fallbacks
//! - [`copy`]: copy policy, file copies, and per-file reports
//! - [`platform`]: per-AI-platform core, prompt, and agent files
//! - [`structure`]: the `.plaesy/` tree itself
//! - [`mcp`]: registration in `.mcp.json`
//! - [`init`]: the `plaesy init` entry point

pub mod config;
pub mod copy;
pub mod home;
pub mod hooks;
pub mod init;
pub mod mcp;
pub mod native_install;
pub mod platform;
pub mod prune;
pub mod reload;
pub mod stophook;
pub mod structure;

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Errors from scaffolding.
#[derive(Debug, Error)]
pub enum ScaffoldError {
    /// A filesystem operation failed.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    /// The selected AI platform is not configured.
    #[error("invalid AI platform: {platform} (available: {available:?})")]
    InvalidPlatform {
        platform: String,
        available: Vec<String>,
    },
    /// No usable Plaesy home could be found or provisioned.
    #[error("{0}")]
    Home(String),
    /// `platform.json` could not be used.
    #[error("{0}")]
    Config(String),
    /// A target directory is missing, not a directory, or not writable.
    #[error("{0}")]
    Target(String),
}

impl ScaffoldError {
    pub(crate) fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

/// What happened to one file during a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Created,
    Updated,
    Unchanged,
    Protected,
    Failed,
}

/// Per-file outcomes accumulated during a run, so a caller can summarise or assert.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileReport {
    /// Paths are shown relative to this base when set.
    pub base: Option<PathBuf>,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: Vec<String>,
    pub protected: Vec<String>,
    pub failed: Vec<String>,
}

impl FileReport {
    /// Records `dst` under `outcome`.
    pub fn record(&mut self, dst: &std::path::Path, outcome: Outcome) {
        let shown = self
            .base
            .as_deref()
            .and_then(|base| dst.strip_prefix(base).ok())
            .unwrap_or(dst)
            .display()
            .to_string();
        match outcome {
            Outcome::Created => self.created.push(shown),
            Outcome::Updated => self.updated.push(shown),
            Outcome::Unchanged => self.unchanged.push(shown),
            Outcome::Protected => self.protected.push(shown),
            Outcome::Failed => self.failed.push(shown),
        }
    }
}

/// How a copy treats a destination that already exists.
///
/// The default policy (skip existing files) is what `init` uses. `reload`
/// overwrites, except for protected paths, which are created when missing but
/// never overwritten once they exist.
#[derive(Debug, Clone, Default)]
pub struct Policy {
    /// Replace an existing destination instead of skipping it.
    pub overwrite: bool,
    /// Report what would happen without writing.
    pub dry_run: bool,
    /// Destinations that are never overwritten once they exist.
    pub protected: std::collections::HashSet<PathBuf>,
    /// Accumulated outcomes.
    pub report: FileReport,
}

impl Policy {
    /// Reports whether `dst` is protected.
    #[must_use]
    pub fn is_protected(&self, dst: &std::path::Path) -> bool {
        !self.protected.is_empty() && self.protected.contains(dst)
    }
}

/// Reports whether `path` exists as a non-directory.
#[must_use]
pub fn file_exists(path: &std::path::Path) -> bool {
    path.metadata().is_ok_and(|m| !m.is_dir())
}

/// Reports whether `dir` contains at least one file anywhere below it.
#[must_use]
pub fn dir_has_files(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            dir_has_files(&path)
        } else {
            true
        }
    })
}
