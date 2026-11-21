//! Unit tests for `tests` split out of `exclude.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn dot_and_known_dependency_directories_are_excluded() {
    assert!(dir_name(".git"));
    assert!(dir_name(".plaesy"));
    assert!(dir_name("node_modules"));
    assert!(!dir_name("src"));
    assert!(!dir_name(""));
}

#[test]
fn flutter_ephemeral_and_generated_assets_are_excluded_by_location() {
    assert!(rel_path("ios/Flutter/ephemeral"));
    assert!(rel_path("linux/flutter/ephemeral"));
    assert!(rel_path("scripts/internal/assets/data/skills/x.md"));
    assert!(!rel_path("scripts/internal/assets/gen"));
}

#[test]
fn gitignore_rules_apply_last_match_wins() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".gitignore"),
        "# comment\n*.log\n!keep.log\n/out/\nsecret/**\n",
    )
    .unwrap();
    let ig = Ignore::load(dir.path());
    assert!(ig.skip_file("debug.log"));
    assert!(ig.skip_file("nested/debug.log"));
    assert!(!ig.skip_file("keep.log"));
    assert!(ig.skip_dir("out", "out"));
    assert!(!ig.skip_file("out"), "dir-only rule must not match a file");
    assert!(
        !ig.skip_file("src/out"),
        "anchored rule only matches at the root"
    );
    assert!(ig.skip_file("secret/key.txt"));
}

#[test]
fn missing_gitignore_ignores_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let ig = Ignore::load(dir.path());
    assert!(!ig.skip_file("anything.log"));
}

#[test]
fn index_walk_enters_only_memory_and_decisions_under_plaesy() {
    let ig = Ignore::default();
    assert!(!ig.skip_index_dir(".plaesy", ".plaesy"));
    assert!(!ig.skip_index_dir(".plaesy/memory", "memory"));
    assert!(ig.skip_index_dir(".plaesy/analysis", "analysis"));
    assert!(!ig.skip_index_file(".plaesy/decisions/x.md"));
    assert!(ig.skip_index_file(".plaesy/context.md"));
}

#[test]
fn bracket_classes_and_escapes_translate() {
    let re = Regex::new(&format!("^{}$", glob_to_regex("file[!0-9].txt"))).unwrap();
    assert!(re.is_match("filex.txt"));
    assert!(!re.is_match("file5.txt"));
    let esc = Regex::new(&format!("^{}$", glob_to_regex(r"a\*b"))).unwrap();
    assert!(esc.is_match("a*b"));
    assert!(!esc.is_match("axb"));
}
