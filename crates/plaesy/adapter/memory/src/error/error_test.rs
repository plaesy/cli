//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn test_error_is_recoverable() {
    let timeout_err = MemoryError::EmbeddingTimeout { timeout_secs: 5 };
    assert!(timeout_err.is_recoverable());

    let db_err = MemoryError::DatabaseError("corruption".to_string());
    assert!(!db_err.is_recoverable());
}

#[test]
fn test_error_user_message() {
    let timeout_err = MemoryError::EmbeddingTimeout { timeout_secs: 5 };
    let msg = timeout_err.user_message();
    assert!(msg.contains("Semantic search"));
    assert!(msg.contains("keyword search"));
}

#[test]
fn test_error_display() {
    let err = MemoryError::DimensionMismatch {
        expected: 384,
        got: 512,
    };
    assert_eq!(
        err.to_string(),
        "Vector dimension mismatch: expected 384, got 512"
    );
}
