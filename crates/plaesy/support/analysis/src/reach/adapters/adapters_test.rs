//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;
use crate::reach::types::Platform;

#[test]
fn every_platform_has_exactly_one_adapter() {
    let adapters = builtin();
    assert_eq!(adapters.len(), Platform::ALL.len());
    for p in Platform::ALL {
        assert_eq!(adapters.iter().filter(|a| a.name() == p).count(), 1, "{p}");
    }
}
