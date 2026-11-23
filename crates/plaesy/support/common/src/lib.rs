//! Utilitas bersama, konfigurasi, dan VERSION.

pub mod common;
pub mod config;
pub mod evidence;
pub mod exclude;
pub mod expected;
pub mod export;
pub mod files;
pub mod imagegen;
pub mod installer;
pub mod mcpserver;
pub mod projects;
pub mod sessions;
pub mod telemetry;
pub mod trimmer;
pub mod workspaces;

/// Release version, read from the repository-root `VERSION` file at build time.
pub const VERSION: &str = include_str!("../../../../../VERSION");

/// Returns the version string, normalized to strict semver.
#[must_use]
pub fn version() -> &'static str {
    common::version::normalize(VERSION)
}
