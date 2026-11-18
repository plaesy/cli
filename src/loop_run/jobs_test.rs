//! Unit tests for `tests` split out of `jobs.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_finished_job_has_its_exit_code_and_output() {
    let dir = tempfile::tempdir().unwrap();
    let argv = if cfg!(windows) {
        vec!["cmd".to_owned(), "/C".into(), "echo hello".into()]
    } else {
        vec!["sh".to_owned(), "-c".into(), "echo hello".into()]
    };
    let job = start(None, dir.path(), "step-1", &argv).unwrap();
    job.wait();
    let result = read_result(dir.path(), "step-1").unwrap();
    assert!(result.finished);
    assert_eq!(result.exit_code, 0);
    assert!(result.output.contains("hello"));
}

#[test]
fn a_job_with_no_exit_file_is_unfinished_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!read_result(dir.path(), "never").unwrap().finished);
}
