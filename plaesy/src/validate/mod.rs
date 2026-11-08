//! Validators for generated documents and the project memory.
//!
//! Port of `scripts/internal/validate` (Go). The OOXML validators check the
//! container and XML structure directly. They do not run the LibreOffice render
//! step, which the Go version also skipped.

pub mod assumptions;
pub mod constitution;
pub mod docx;
pub mod memory;
pub mod ooxml;
pub mod pptx;
pub mod xlsx;

use thiserror::Error;

/// A validation failure, or a file that could not be checked at all.
#[derive(Debug, Error)]
pub enum ValidateError {
    /// The document does not satisfy the rules. The message lists every problem.
    #[error("{0}")]
    Invalid(String),
    /// A file or directory could not be read.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

impl ValidateError {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

/// Matches an unfilled `{{ ... }}` template tag.
pub(crate) fn jinja_tag_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"\{\{.*?\}\}").expect("static regex"))
}
