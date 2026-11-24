//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn test_prelude_includes_error() {
    // Verify prelude is usable
    let _error_type: std::result::Result<(), MemoryError> = Ok(());
}

#[test]
fn test_module_structure() {
    // Verify all modules are accessible
    let _ = error::MemoryError::InternalError("test".to_string());
}
