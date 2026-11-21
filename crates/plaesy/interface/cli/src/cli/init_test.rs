//! Unit tests for `tests` split out of `init.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn creates_marker_and_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    // A `.git` directory marks this temp dir as its own project boundary, so the
    // nested-project check does not walk up into a parent `.plaesy` (e.g. in the home dir).
    fs::create_dir(temp.path().join(".git")).unwrap();
    let args = Args {
        path: temp.path().to_path_buf(),
        dry_run: false,
    };
    run(&args).unwrap();
    assert!(temp.path().join(".plaesy").is_dir());
    fs::write(temp.path().join(".plaesy/keep.txt"), "keep").unwrap();
    run(&args).unwrap();
    assert_eq!(
        fs::read_to_string(temp.path().join(".plaesy/keep.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn dry_run_does_not_write() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join(".git")).unwrap();
    run(&Args {
        path: temp.path().to_path_buf(),
        dry_run: true,
    })
    .unwrap();
    assert!(!temp.path().join(".plaesy").exists());
}

#[test]
fn refuses_nested_project_root() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join(".plaesy")).unwrap();
    let nested = temp.path().join("nested");
    fs::create_dir(&nested).unwrap();
    assert!(run(&Args {
        path: nested.clone(),
        dry_run: false
    })
    .is_err());
    assert!(!nested.join(".plaesy").exists());
}

#[test]
fn allows_independent_nested_git_project() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join(".plaesy")).unwrap();
    let nested = temp.path().join("nested");
    fs::create_dir_all(nested.join(".git")).unwrap();
    run(&Args {
        path: nested.clone(),
        dry_run: false,
    })
    .unwrap();
    assert!(nested.join(".plaesy").is_dir());
}

#[cfg(unix)]
#[test]
fn refuses_symlink_instead_of_treating_it_as_project_directory() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    fs::create_dir(&target).unwrap();
    symlink(&target, temp.path().join(".plaesy")).unwrap();
    assert!(run(&Args {
        path: temp.path().to_path_buf(),
        dry_run: false
    })
    .is_err());
    assert!(fs::read_dir(&target).unwrap().next().is_none());
}
