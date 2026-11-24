//! Konteks domain flow (ADR 0002, ADR 0003).
//!
//! Modul:
//! - `ir`: nama dan pengenal dokumen `flow-ir/1`.
//! - `semantic`: aturan semantik yang tidak bisa dinyatakan JSON Schema.
//! - `error`: tipe galat domain.
//!
//! Crate ini tidak boleh bergantung pada tokio, axum, rusqlite, atau crate I/O lain.

pub mod error;
pub mod eval;
pub mod ir;
pub mod port;
pub mod semantic;

pub use error::SemanticError;
pub use semantic::validate;
