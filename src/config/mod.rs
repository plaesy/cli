//! Configuration: the platform registry (`configs/platform.json`), the unified
//! `.plaesy/config.json` schema, and delegate provider settings.
//!
//! Port of `scripts/internal/config` (Go).

pub mod delegate;
pub mod manager;
pub mod schema;

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Errors raised while reading or writing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The configuration file does not exist.
    #[error("platform configuration file not found: {}", .0.display())]
    NotFound(PathBuf),
    /// The file exists but is not valid JSON for the expected shape.
    #[error("invalid JSON syntax in platform configuration: {0}")]
    Json(#[from] serde_json::Error),
    /// A file could not be read or written.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    /// A platform id or display name is not in the registry.
    #[error("unknown platform: {0}")]
    UnknownPlatform(String),
    /// Cleanup needs at least one directory and none could be derived.
    #[error("no clean directories found")]
    NoCleanDirectories,
    /// A structure component name is not recognised.
    #[error(
        "unknown plaesy structure component: {0} (available: base_directory, core_directories, project_directories, memory_subdirectories)"
    )]
    UnknownComponent(String),
    /// A user-supplied migration function rejected the old file.
    #[error("migrating config: {0}")]
    Migration(String),
}

impl ConfigError {
    fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}
