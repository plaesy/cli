//! Unit tests for `tests` split out of `types.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn node_type_follows_the_top_folder_then_the_extension() {
    assert_eq!(node_type_of("agents/x.agents.md"), "agent");
    assert_eq!(node_type_of("src/main.rs"), "source");
    assert_eq!(node_type_of("README.txt"), "other");
}

#[test]
fn architecture_layer_reads_folders_then_names() {
    assert_eq!(architecture_layer("api/users.go"), "API");
    assert_eq!(architecture_layer("models/user.rs"), "Data");
    assert_eq!(architecture_layer("docs/readme.md"), "");
}
