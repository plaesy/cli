//! Offline version comparison for `plaesy update --check`.
//!
//! The latest version comes from a local JSON manifest (`{"version": "x.y.z"}`) and the installed
//! version from running the installed binary with `--version`. Nothing here touches the network;
//! downloading a release is not implemented.

use std::cmp::Ordering;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;

use serde::Deserialize;
use thiserror::Error;

use super::{installed_path, InstallerError};

/// Errors from reading a manifest or probing the installed binary.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// A version string is not strict `MAJOR.MINOR.PATCH` (an optional leading `v` is allowed).
    #[error("'{0}' is not a MAJOR.MINOR.PATCH version")]
    InvalidVersion(String),
    /// The manifest file could not be read.
    #[error("cannot read manifest {path}: {source}")]
    ManifestRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// The manifest file is not JSON with a `version` string.
    #[error("cannot parse manifest {path}: {source}")]
    ManifestParse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    /// The installed binary could not be started.
    #[error("cannot run installed binary {path}: {source}")]
    Probe {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// The installed binary ran but `--version` did not succeed.
    #[error("installed binary {path} exited with {status} for --version")]
    ProbeFailed { path: String, status: String },
    /// The installed binary printed no version token.
    #[error("installed binary {path} printed no version")]
    ProbeOutput { path: String },
    /// The installed binary path could not be resolved.
    #[error(transparent)]
    Installer(#[from] InstallerError),
}

/// A strict `MAJOR.MINOR.PATCH` release version. Field order gives semver ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn number(part: &str) -> Option<u64> {
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

impl FromStr for Version {
    type Err = UpdateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let body = trimmed.strip_prefix('v').unwrap_or(trimmed);
        let invalid = || UpdateError::InvalidVersion(s.trim().to_owned());
        let parts: Vec<&str> = body.split('.').collect();
        let [major, minor, patch] = parts.as_slice() else {
            return Err(invalid());
        };
        Ok(Self {
            major: number(major).ok_or_else(invalid)?,
            minor: number(minor).ok_or_else(invalid)?,
            patch: number(patch).ok_or_else(invalid)?,
        })
    }
}

/// Result of comparing the installed version with the latest known version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable,
    InstalledIsNewer,
}

/// Compares the installed version with the latest version from the manifest.
#[must_use]
pub fn compare(installed: Version, latest: Version) -> UpdateStatus {
    match installed.cmp(&latest) {
        Ordering::Equal => UpdateStatus::UpToDate,
        Ordering::Less => UpdateStatus::UpdateAvailable,
        Ordering::Greater => UpdateStatus::InstalledIsNewer,
    }
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
}

/// Reads the `version` field of a local JSON manifest.
///
/// # Errors
///
/// The file cannot be read or parsed, or its version is not `MAJOR.MINOR.PATCH`.
pub fn manifest_version(path: &Path) -> Result<Version, UpdateError> {
    let shown = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|source| UpdateError::ManifestRead {
        path: shown.clone(),
        source,
    })?;
    let manifest: Manifest =
        serde_json::from_str(&text).map_err(|source| UpdateError::ManifestParse {
            path: shown,
            source,
        })?;
    manifest.version.parse()
}

/// Version reported by the installed binary, or `None` when nothing is installed.
///
/// Runs the installed binary with `--version` and reads the last token of its output.
///
/// # Errors
///
/// The install path cannot be resolved, the binary cannot be started or exits with failure, or
/// its output has no parsable version.
pub fn installed_version() -> Result<Option<Version>, UpdateError> {
    let path = installed_path()?;
    if !path.is_file() {
        return Ok(None);
    }
    let shown = path.display().to_string();
    let output = Command::new(&path)
        .arg("--version")
        .output()
        .map_err(|source| UpdateError::Probe {
            path: shown.clone(),
            source,
        })?;
    if !output.status.success() {
        let status = output
            .status
            .code()
            .map_or_else(|| "no exit code".to_owned(), |code| code.to_string());
        return Err(UpdateError::ProbeFailed {
            path: shown,
            status,
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let token = text
        .split_whitespace()
        .last()
        .ok_or_else(|| UpdateError::ProbeOutput {
            path: shown.clone(),
        })?;
    token.parse::<Version>().map(Some)
}

#[cfg(test)]
#[path = "update_test.rs"]
mod update_test;
