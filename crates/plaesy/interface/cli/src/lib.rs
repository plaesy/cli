//! Antarmuka baris perintah (ADR 0003: interface/cli).

pub mod cli;

// Sibling crates, aliased so that `crate::<module>` paths keep resolving.
pub(crate) use plaesy_adapter_memory::decisions;
pub(crate) use plaesy_adapter_memory::memory;
pub(crate) use plaesy_adapter_relay::relay;
#[allow(unused_imports)]
pub(crate) use plaesy_analysis::analyze;
pub(crate) use plaesy_analysis::graph;
pub(crate) use plaesy_analysis::reach;
pub(crate) use plaesy_analysis::validate;
pub(crate) use plaesy_common::common;
pub(crate) use plaesy_common::config;
pub(crate) use plaesy_common::files;
pub(crate) use plaesy_common::imagegen;
pub(crate) use plaesy_common::installer;
pub(crate) use plaesy_common::mcpserver;
pub(crate) use plaesy_common::projects;
pub(crate) use plaesy_common::sessions;
pub(crate) use plaesy_common::trimmer;
pub(crate) use plaesy_common::version;
pub(crate) use plaesy_designlint::designlint;
pub(crate) use plaesy_designlint::evals;
pub(crate) use plaesy_loop::delegate;
pub(crate) use plaesy_loop::loop_run;
pub(crate) use plaesy_mdlint::mdlint;
pub(crate) use plaesy_tasks::checklist;
pub(crate) use plaesy_tasks::tasks;
