//! Unit tests for `chat_subagent.rs`, included via `#[path]`.

use std::fs;
use std::process::Command;

use super::*;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

#[test]
fn a_worktree_holds_the_committed_files_and_leaves_the_main_checkout_alone() {
    let repo = tempfile::tempdir().expect("tempdir");
    git(repo.path(), &["init", "-q"]);
    fs::write(repo.path().join("a.txt"), "isi").expect("write");
    git(repo.path(), &["add", "a.txt"]);
    git(
        repo.path(),
        &[
            "-c",
            "user.name=tes",
            "-c",
            "user.email=tes@example.invalid",
            "commit",
            "-q",
            "-m",
            "awal",
        ],
    );
    let dest = dir(repo.path()).join("sub-tes");
    create_worktree(repo.path(), &dest).expect("worktree");
    assert_eq!(fs::read_to_string(dest.join("a.txt")).expect("read"), "isi");
    fs::write(dest.join("b.txt"), "baru").expect("write");
    assert!(!repo.path().join("b.txt").exists());
}

#[test]
fn a_name_is_a_subagent_name() {
    assert!(new_name().starts_with("sub-"));
}
