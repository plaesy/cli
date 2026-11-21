//! File-backed run log (adapter). Implements the `RunSink` port of `plaesy-domain-workflow`.

pub mod file;

pub use file::{checksum, RunLog};
pub use plaesy_domain_workflow::runstore::{Recovered, RunLogError, RunSink};
