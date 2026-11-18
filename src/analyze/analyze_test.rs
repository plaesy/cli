//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn symbol_names_normalize_across_conventions() {
    assert_eq!(
        normalize_symbol_name("getUserById"),
        normalize_symbol_name("get_user_by_id")
    );
    assert_eq!(normalize_symbol_name("GetUserById"), "getuserbyid");
}

#[test]
fn repeated_locations_collapse_with_a_count() {
    assert_eq!(
        format_locations(&["a.go", "a.go", "b.go"]),
        "a.go (x2), b.go"
    );
}

#[test]
fn a_run_writes_the_summary_artifacts() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("main.rs"), "fn main() {}").expect("write");
    let opts = Options {
        project_path: Some(dir.path().to_path_buf()),
        no_graph: true,
        force: true,
        if_changed: false,
    };
    let log = Logger::new("test");
    run(&opts, &log).expect("run");
    let analysis = dir.path().join(".plaesy").join("analysis");
    assert!(analysis.join("project.json").is_file());
    assert!(analysis.join("project.structure.json").is_file());
    assert!(analysis.join("overview.md").is_file());
}
