//! Analisis repo, graf, validasi, dan jangkauan (belum ada konteks di ADR 0003).

pub mod analyze;
pub mod graph;
pub mod reach;
pub mod validate;

// Sibling crates, aliased so that `crate::<module>` paths keep resolving.
#[allow(unused_imports)]
pub(crate) use plaesy_common::common;
pub(crate) use plaesy_common::config;
pub(crate) use plaesy_common::exclude;
pub(crate) use plaesy_common::version;
