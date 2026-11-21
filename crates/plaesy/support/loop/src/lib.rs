//! Delegasi dan loop eksekusi (ADR 0003: workflow).

pub mod delegate;
pub mod loop_run;

// Sibling crates, aliased so that `crate::<module>` paths keep resolving.
pub(crate) use plaesy_common::config;
#[allow(unused_imports)]
pub(crate) use plaesy_tasks::checklist;
