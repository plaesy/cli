//! Konteks domain app (ADR 0003). Aturan semantik app-ir/1 (ADR 0006) (lihat ADR terkait).
//! Port dari validator Rust di `src/irvalidate`; pemeriksaan bentuk dilakukan JSON Schema.
//! Crate ini tidak boleh bergantung pada tokio, axum, rusqlite, atau crate I/O lain.

pub mod bindings;
pub mod navigation;
pub mod rules;
pub mod screens;

use plaesy_domain_flow::SemanticError;
pub use plaesy_kernel::json;
use serde_json::Value;

/// Returns every semantic error in `doc`. An empty vector means the document passes.
pub fn validate(doc: &Value) -> Vec<SemanticError> {
    rules::validate(doc)
        .into_iter()
        .map(SemanticError::new)
        .collect()
}
