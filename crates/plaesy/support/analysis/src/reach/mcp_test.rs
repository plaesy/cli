//! Unit tests for `tests` split out of `mcp.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn the_default_manager_knows_the_four_mcporter_servers() {
    let m = default_manager();
    for name in ["exa", "linkedin", "xiaohongshu", "xiaoyuzhou"] {
        assert!(m.get(name).is_some(), "{name}");
    }
    assert!(m.get("missing").is_none());
}

#[test]
fn calling_an_unregistered_server_is_an_error() {
    let m = McpManager::default();
    let deadline = Deadline::after(std::time::Duration::from_secs(1));
    assert!(m.call("nope", "x", Map::new(), deadline).is_err());
}
