//! Structured citations for search results (EV-01).

use super::*;

/// Citation builder: constructs structured citations for search results (EV-01 compliance).
pub struct CitationBuilder {}

impl CitationBuilder {
    /// Create a new citation builder.
    pub fn new() -> Self {
        CitationBuilder {}
    }

    /// Build a citation for a search result.
    ///
    /// # Errors
    ///
    /// - `CitationError` if the result lacks a source path, has an invalid
    ///   line range (must be 1-based and `start <= end`), or has no timestamp
    pub fn build(&self, result: &SearchResult) -> Result<Citation> {
        if result.source_path.trim().is_empty() {
            return Err(MemoryError::CitationError(format!(
                "chunk {} has no source path",
                result.chunk_id
            )));
        }
        if result.start_line == 0 || result.end_line < result.start_line {
            return Err(MemoryError::CitationError(format!(
                "chunk {} has invalid line range {}-{}",
                result.chunk_id, result.start_line, result.end_line
            )));
        }
        if result.indexed_at.trim().is_empty() {
            return Err(MemoryError::CitationError(format!(
                "chunk {} has no index timestamp",
                result.chunk_id
            )));
        }
        let docid = docid_for(&result.source_path);
        Ok(Citation {
            chunk_id: result.chunk_id,
            docid,
            file_path: result.source_path.clone(),
            revision: result.revision.clone(),
            line_range: (result.start_line, result.end_line),
            timestamp: result.indexed_at.clone(),
            confidence: match result.confidence {
                ConfidenceLevel::Fact => "FACT",
                ConfidenceLevel::Inferred => "INFERRED",
                ConfidenceLevel::Speculative => "SPECULATIVE",
            }
            .to_string(),
            source: result.meta.clone(),
        })
    }
}

impl Default for CitationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Structured citation for search results (EV-01 compliance).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Citation {
    /// Chunk the citation points at.
    pub chunk_id: u64,

    /// Docid derived from the normalized relative path (first 6 hex chars of SHA-256).
    pub docid: String,

    /// Source file path (relative to .plaesy/).
    pub file_path: String,

    /// Git revision (if available).
    pub revision: Option<String>,

    /// Line range in source file (1-based, inclusive).
    pub line_range: (usize, usize),

    /// Timestamp when content was indexed (RFC 3339).
    pub timestamp: String,

    /// Confidence in the citation (FACT, INFERRED, SPECULATIVE).
    pub confidence: String,

    /// What the source itself declares: tier, trust, last verification, status, staleness.
    #[serde(default)]
    pub source: ResultMeta,
}

impl std::fmt::Display for Citation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{} [{}] indexed {}",
            self.file_path, self.line_range.0, self.line_range.1, self.confidence, self.timestamp
        )?;
        if let Some(rev) = &self.revision {
            write!(f, " @ {rev}")?;
        }
        if !self.docid.is_empty() {
            write!(f, " {}", self.docid)?;
        }
        let m = &self.source;
        let mut notes: Vec<String> = Vec::new();
        if let Some(t) = &m.tier {
            notes.push(format!("tier {t}"));
        }
        if let Some(t) = &m.trust {
            notes.push(format!("trust {t}"));
        }
        if let Some(v) = &m.last_verified {
            notes.push(format!("verified {v}"));
        }
        if let Some(s) = &m.status {
            if s != "active" {
                notes.push(format!("status {s}"));
            }
        }
        if m.stale {
            notes.push("STALE".to_string());
        }
        if !notes.is_empty() {
            write!(f, " ({})", notes.join(", "))?;
        }
        Ok(())
    }
}

/// Derive a short docid from a normalized relative path (first 6 hex chars of SHA-256).
pub(super) fn docid_for(path: &str) -> String {
    use sha2::{Digest, Sha256};
    let normalized = path.replace('\\', "/").trim().to_lowercase();
    let digest = Sha256::digest(normalized.as_bytes());
    digest.iter().map(|b| format!("{b:x}")).collect::<String>()[..6].to_string()
}
