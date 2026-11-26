//! Task states for the in-house agent protocol. The state machine lives in the relay adapter, next
//! to the storage that enforces it; this module re-exports it for the CLI.

pub use plaesy_adapter_relay::task::{transition, TaskState};

#[cfg(test)]
#[path = "agent_task_test.rs"]
mod agent_task_test;
