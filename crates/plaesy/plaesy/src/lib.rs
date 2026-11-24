//! Plaesy CLI: a facade over the `plaesy-*` crates under `crates/plaesy/`.
//!
//! Each module is re-exported from the crate that owns it, so `plaesy::<module>`
//! keeps its path for the binary, the integration tests, and the examples.

pub use plaesy_adapter_design_ir::{designir, irvalidate};
pub use plaesy_adapter_memory::{decisions, memory};
pub use plaesy_adapter_relay::relay;
pub use plaesy_adapter_runtime::{adapter, runtime};
pub use plaesy_analysis::{analyze, graph, reach, validate};
pub use plaesy_common::{
    common, config, evidence, exclude, expected, files, imagegen, installer, mcpserver, projects,
    sessions, trimmer,
};
pub use plaesy_common::{version, VERSION};
pub use plaesy_designlint::{designlint, evals};
pub use plaesy_domain_identity::{authz, identity, secrets, users};
pub use plaesy_domain_workflow::{policy, runstore, task_run};
pub use plaesy_interface_cli::cli;
pub use plaesy_loop::{delegate, loop_run};
pub use plaesy_mdlint::mdlint;
pub use plaesy_tasks::{checklist, tasks};
