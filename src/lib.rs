//! Plaesy CLI for memory and agent workflows.
//!
//! Porting is incremental. Each Go package maps to one module here; see
//! `PORTING.md` for the status of every package.

pub mod analyze;
pub mod checklist;
pub mod cli;
pub mod common;
pub mod config;
pub mod delegate;
pub mod designlint;
pub mod evals;
pub mod exclude;
pub mod expected;
pub mod graph;
pub mod imagegen;
pub mod installer;
pub mod loop_run;
pub mod mcpserver;
pub mod mdlint;
pub mod memory;
pub mod reach;
pub mod tasks;
pub mod trimmer;
pub mod validate;

/// Release version, read from the repository-root `VERSION` file at build time.
pub const VERSION: &str = include_str!("../VERSION");

/// Returns the version string, normalized to strict semver.
#[must_use]
pub fn version() -> &'static str {
    common::version::normalize(VERSION)
}
