//! The autonomous loop: runs one step per iteration until a Stop or Completion
//! condition holds, and keeps the run state in `.plaesy/state.json`.
//!
//! The driver never asks a question. Each decision that a config default settles is
//! recorded in the report instead.
//! Port of `scripts/internal/loop` (Go). Named `loop_run` because `loop` is a Rust keyword.

pub mod completion;
pub mod config;
pub mod driver;
pub mod jobs;
pub mod measure;
pub mod queue;
pub mod receipt;
pub mod state;
pub mod step_adapter;

pub use crate::scaffold::stophook::{COMPLETION_REPORT_FILE, STOP_REPORT_FILE};
