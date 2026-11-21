//! Kontrak adapter eksekusi dan runtime lokal (ADR 0003: adapter/runtime).

pub mod adapter;
pub mod runtime;

// Sibling crates, aliased so that `crate::<module>` paths keep resolving.
#[allow(unused_imports)]
pub(crate) use plaesy_domain_workflow::policy;
