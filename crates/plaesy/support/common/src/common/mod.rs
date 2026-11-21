//! Shared logging, validation, version, and git helpers.
//!
//! Port of `scripts/internal/common` (Go). Logging is carried by a [`Logger`]
//! value instead of package-level globals, so each caller decides where output
//! goes and tests can capture it.

pub mod banner;
pub mod git;
pub mod log;
pub mod validate;
pub mod version;

pub use log::{Level, Logger};
pub use validate::ValidationError;
