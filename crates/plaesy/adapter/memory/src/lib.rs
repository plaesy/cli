//! Mesin memori: SQLite, FTS5, HNSW, ONNX (ADR 0003: adapter).

pub mod decisions;
pub mod memory;

// Sibling crates, aliased so that `crate::<module>` paths keep resolving.
#[allow(unused_imports)]
pub(crate) use plaesy_mdlint::mdlint;
